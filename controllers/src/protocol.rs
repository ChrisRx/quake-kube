//! Quake 3 out-of-band (connectionless) packets, as used between servers, clients and the
//! master server.

use rand::distr::{Alphanumeric, SampleString};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};

/// Every out-of-band packet starts with this marker.
pub const OOB_PREFIX: &[u8] = b"\xff\xff\xff\xff";

/// Returns the command text of an out-of-band packet.
pub fn parse_oob(packet: &[u8]) -> Option<&str> {
    std::str::from_utf8(packet.strip_prefix(OOB_PREFIX)?).ok()
}

/// Wraps data in an out-of-band packet.
pub fn oob(data: &[u8]) -> Vec<u8> {
    [OOB_PREFIX, data].concat()
}

/// Parses an info string of the form `\key\value\key\value`.
pub fn parse_info(s: &str) -> BTreeMap<String, String> {
    let mut parts = s.trim_end_matches(['\n', '\0']).split('\\');
    // The string starts with a separator, so the first part is empty.
    parts.next();
    let mut info = BTreeMap::new();
    while let (Some(key), Some(value)) = (parts.next(), parts.next()) {
        info.insert(key.to_owned(), value.to_owned());
    }
    info
}

/// Sends `getinfo` to a server and returns its info string. Fails if the server doesn't answer
/// within the timeout or echoes the wrong challenge.
pub async fn get_info(
    addr: SocketAddr,
    wait: Duration,
) -> std::io::Result<BTreeMap<String, String>> {
    let bind: SocketAddr = if addr.is_ipv4() {
        ([0, 0, 0, 0], 0).into()
    } else {
        ([0u16; 8], 0).into()
    };
    let socket = UdpSocket::bind(bind).await?;
    socket.connect(addr).await?;
    let challenge = Alphanumeric.sample_string(&mut rand::rng(), 12);
    socket
        .send(&oob(format!("getinfo {challenge}").as_bytes()))
        .await?;

    let mut buf = [0u8; 2048];
    let deadline = tokio::time::Instant::now() + wait;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let len = timeout(remaining, socket.recv(&mut buf))
            .await
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::TimedOut))??;
        let Some(info) = parse_oob(&buf[..len])
            .and_then(|cmd| cmd.strip_prefix("infoResponse\n"))
            .map(parse_info)
        else {
            continue;
        };
        if info.get("challenge") == Some(&challenge) {
            return Ok(info);
        }
    }
}

/// Encodes a server list response, split into packets small enough to avoid fragmentation.
/// IPv6 addresses are only sent in extended responses.
pub fn servers_response(servers: &[SocketAddr], extended: bool) -> Vec<Vec<u8>> {
    const MAX_PACKET: usize = 1400;
    const EOT: &[u8] = b"\\EOT\0\0\0";
    let header = oob(if extended {
        b"getserversExtResponse"
    } else {
        b"getserversResponse"
    });

    let mut packets = Vec::new();
    let mut packet = header.clone();
    for server in servers {
        let mut entry = Vec::with_capacity(19);
        match server {
            SocketAddr::V4(addr) => {
                entry.push(b'\\');
                entry.extend_from_slice(&addr.ip().octets());
            }
            SocketAddr::V6(addr) if extended => {
                entry.push(b'/');
                entry.extend_from_slice(&addr.ip().octets());
            }
            SocketAddr::V6(_) => continue,
        }
        entry.extend_from_slice(&server.port().to_be_bytes());
        if packet.len() + entry.len() + EOT.len() > MAX_PACKET {
            packet.extend_from_slice(EOT);
            packets.push(std::mem::replace(&mut packet, header.clone()));
        }
        packet.extend_from_slice(&entry);
    }
    packet.extend_from_slice(EOT);
    packets.push(packet);
    packets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_info_response() {
        let info = parse_info("\\challenge\\abc\\hostname\\quake kube\\clients\\3\n");
        assert_eq!(info["challenge"], "abc");
        assert_eq!(info["hostname"], "quake kube");
        assert_eq!(info["clients"], "3");
    }

    #[test]
    fn encodes_servers() {
        let servers = [
            "10.0.0.1:27960".parse().unwrap(),
            "[fd00::1]:27961".parse().unwrap(),
        ];

        let packets = servers_response(&servers, false);
        assert_eq!(
            packets,
            vec![[
                &b"\xff\xff\xff\xffgetserversResponse"[..],
                b"\\\x0a\x00\x00\x01\x6d\x38",
                b"\\EOT\0\0\0",
            ]
            .concat()]
        );

        let packets = servers_response(&servers, true);
        assert_eq!(packets.len(), 1);
        assert!(packets[0].starts_with(b"\xff\xff\xff\xffgetserversExtResponse\\"));
        assert!(packets[0].windows(2).any(|w| w == b"\x6d\x39"));
    }

    #[test]
    fn splits_large_responses() {
        let servers: Vec<SocketAddr> = (0..500u16)
            .map(|i| SocketAddr::from(([10, 0, (i >> 8) as u8, i as u8], 27960)))
            .collect();
        let packets = servers_response(&servers, false);
        assert!(packets.len() > 1);
        assert!(packets
            .iter()
            .all(|p| p.len() <= 1400 && p.ends_with(b"\\EOT\0\0\0")));
        let entries: usize = packets.iter().map(|p| (p.len() - 22 - 7) / 7).sum();
        assert_eq!(entries, 500);
    }
}
