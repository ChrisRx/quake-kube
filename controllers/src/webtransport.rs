//! WebTransport for browser clients. Game traffic goes in unreliable datagrams, like native
//! clients' UDP, instead of over a WebSocket where one lost packet holds up everything behind
//! it. Packets too large for a datagram go over a stream the client opens, framed with a
//! 2-byte big-endian length.

use crate::config::WebTransportConfig;
use crate::prelude::*;
use crate::quake_server::GAME_PORT;
use crate::registry;
use kube::runtime::reflector::{ObjectRef, Store};
use std::sync::RwLock;
use std::time::SystemTime;
use tokio::net::UdpSocket;
use tracing::*;
use wtransport::endpoint::endpoint_side::Server;
use wtransport::endpoint::IncomingSession;
use wtransport::error::SendDatagramError;
use wtransport::{Connection, Endpoint, Identity, RecvStream, SendStream, ServerConfig};

/// Browsers only accept self-signed certificates by hash when they're valid for at most 14
/// days, so they're replaced well before then.
const SELF_SIGNED_LIFETIME: Duration = Duration::from_secs(10 * 24 * 60 * 60);

/// How often to check whether the certificate needs reloading or replacing.
const CERTIFICATE_CHECK: Duration = Duration::from_secs(60);

pub struct WebTransport {
    endpoint: Endpoint<Server>,
    config: WebTransportConfig,
    /// Hashes browsers need to accept the self-signed certificate. Empty when browsers trust
    /// the certificate.
    certificate_hashes: RwLock<Vec<[u8; 32]>>,
}

impl WebTransport {
    pub async fn new(config: WebTransportConfig) -> anyhow::Result<Arc<Self>> {
        let identity = load_identity(&config).await?;
        let hashes = self_signed_hashes(&config, &identity);
        let endpoint = Endpoint::server(server_config(&config, identity))?;
        info!("webtransport listening on {}", endpoint.local_addr()?);
        Ok(Arc::new(WebTransport {
            endpoint,
            config,
            certificate_hashes: RwLock::new(hashes),
        }))
    }

    pub fn public_address(&self) -> Option<&str> {
        self.config.public_address.as_deref()
    }

    pub fn public_port(&self) -> u16 {
        self.config.public_port
    }

    pub fn certificate_hashes(&self) -> Vec<[u8; 32]> {
        self.certificate_hashes.read().unwrap().clone()
    }

    /// Accepts sessions for /play/<namespace>/<name>, relaying each to that server.
    pub async fn serve(self: Arc<Self>, store: Store<QuakeServer>) {
        loop {
            let incoming = self.endpoint.accept().await;
            let store = store.clone();
            tokio::spawn(async move {
                if let Err(error) = handle_session(incoming, store).await {
                    debug!("webtransport session ended: {error}");
                }
            });
        }
    }

    /// Replaces the self-signed certificate before it expires, or reloads the configured one
    /// when it changes, without dropping open sessions.
    pub async fn rotate_certificates(self: Arc<Self>) {
        let mut loaded = (SystemTime::now(), self.certificate_modified());
        loop {
            tokio::time::sleep(CERTIFICATE_CHECK).await;
            let modified = self.certificate_modified();
            let due = match &self.config.tls_dir {
                Some(_) => modified != loaded.1,
                None => loaded.0.elapsed().unwrap_or_default() >= SELF_SIGNED_LIFETIME,
            };
            if !due {
                continue;
            }
            let result = async {
                let identity = load_identity(&self.config).await?;
                let hashes = self_signed_hashes(&self.config, &identity);
                self.endpoint
                    .reload_config(server_config(&self.config, identity), false)?;
                *self.certificate_hashes.write().unwrap() = hashes;
                anyhow::Ok(())
            }
            .await;
            match result {
                Ok(()) => {
                    info!("webtransport certificate replaced");
                    loaded = (SystemTime::now(), modified);
                }
                Err(error) => warn!("replacing webtransport certificate failed: {error}"),
            }
        }
    }

    fn certificate_modified(&self) -> Option<SystemTime> {
        let dir = self.config.tls_dir.as_ref()?;
        std::fs::metadata(dir.join("tls.crt")).ok()?.modified().ok()
    }
}

async fn load_identity(config: &WebTransportConfig) -> anyhow::Result<Identity> {
    Ok(match &config.tls_dir {
        Some(dir) => Identity::load_pemfiles(dir.join("tls.crt"), dir.join("tls.key")).await?,
        None => {
            let host = config
                .public_address
                .as_deref()
                .and_then(|addr| addr.rsplit_once(':').map(|(host, _)| host))
                .unwrap_or("quake-kube");
            Identity::self_signed([host.trim_matches(['[', ']'])])?
        }
    })
}

fn self_signed_hashes(config: &WebTransportConfig, identity: &Identity) -> Vec<[u8; 32]> {
    if config.tls_dir.is_some() {
        return Vec::new();
    }
    identity
        .certificate_chain()
        .as_slice()
        .iter()
        .map(|cert| *cert.hash().as_ref())
        .collect()
}

fn server_config(config: &WebTransportConfig, identity: Identity) -> ServerConfig {
    ServerConfig::builder()
        .with_bind_address(([0, 0, 0, 0], config.port).into())
        .with_identity(identity)
        .keep_alive_interval(Some(Duration::from_secs(3)))
        .build()
}

async fn handle_session(
    incoming: IncomingSession,
    store: Store<QuakeServer>,
) -> anyhow::Result<()> {
    let request = incoming.await?;
    let server = request
        .path()
        .strip_prefix("/play/")
        .and_then(|rest| rest.trim_end_matches('/').split_once('/'))
        .map(|(namespace, name)| (namespace.to_owned(), name.to_owned()))
        .filter(|(namespace, name)| store.get(&ObjectRef::new(name).within(namespace)).is_some());
    // Sessions are only relayed to known servers, so this can't reach anything else in the
    // cluster.
    let Some((namespace, name)) = server else {
        request.not_found().await;
        return Ok(());
    };
    let target = format!("{name}.{namespace}.svc:{GAME_PORT}");
    let connection = request.accept().await?;
    debug!(%target, "browser client connected over webtransport");

    let addr = registry::lookup(&target).await?;
    let udp = UdpSocket::bind(if addr.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    })
    .await?;
    udp.connect(addr).await?;
    relay(connection, Arc::new(udp)).await
}

async fn relay(connection: Connection, udp: Arc<UdpSocket>) -> anyhow::Result<()> {
    let mut large_packets: Option<SendStream> = None;
    let mut buf = vec![0u8; 65536];
    loop {
        tokio::select! {
            datagram = connection.receive_datagram() => {
                udp.send(&datagram?).await?;
            }
            stream = connection.accept_bi(), if large_packets.is_none() => {
                let (send, recv) = stream?;
                debug!("client opened a stream for large packets");
                large_packets = Some(send);
                tokio::spawn(read_large_packets(recv, udp.clone()));
            }
            len = udp.recv(&mut buf) => {
                let packet = &buf[..len?];
                match connection.send_datagram(packet) {
                    Ok(()) => {}
                    Err(SendDatagramError::TooLarge) => match large_packets.as_mut() {
                        Some(stream) => {
                            trace!(len = packet.len(), "sending packet over stream");
                            stream.write_all(&(packet.len() as u16).to_be_bytes()).await?;
                            stream.write_all(packet).await?;
                        }
                        None => debug!(len = packet.len(), "dropping packet too large for a datagram"),
                    },
                    Err(error) => return Err(error.into()),
                }
            }
        }
    }
}

/// Forwards length-prefixed packets the client sent over its stream.
async fn read_large_packets(mut stream: RecvStream, udp: Arc<UdpSocket>) -> anyhow::Result<()> {
    let mut len = [0u8; 2];
    let mut packet = vec![0u8; u16::MAX as usize];
    loop {
        stream.read_exact(&mut len).await?;
        let packet = &mut packet[..u16::from_be_bytes(len) as usize];
        stream.read_exact(packet).await?;
        udp.send(packet).await?;
    }
}
