//! Quake 3 out-of-band (connectionless) packets, as used between servers, clients and the
//! master server.

use rand::distr::{Alphanumeric, SampleString};
use serde::Serialize;
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
    let challenge = Alphanumeric.sample_string(&mut rand::rng(), 12);
    query(addr, wait, &format!("getinfo {challenge}"), |cmd| {
        let info = parse_info(cmd.strip_prefix("infoResponse\n")?);
        (info.get("challenge") == Some(&challenge)).then_some(info)
    })
    .await
}

/// A player in a server's status. Bots are only told apart by their ping, which ioquake3
/// reports as 0 for them.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Player {
    pub name: String,
    pub score: i32,
    pub ping: i32,
}

/// A server's status: its settings and who's playing.
#[derive(Clone, Debug, Default)]
pub struct Status {
    pub info: BTreeMap<String, String>,
    pub players: Vec<Player>,
}

/// Sends `getstatus` to a server and returns its settings and players.
pub async fn get_status(addr: SocketAddr, wait: Duration) -> std::io::Result<Status> {
    let challenge = Alphanumeric.sample_string(&mut rand::rng(), 12);
    query(addr, wait, &format!("getstatus {challenge}"), |cmd| {
        let status = parse_status(cmd.strip_prefix("statusResponse\n")?);
        // Older servers don't echo the challenge.
        let echoed = status.info.get("challenge");
        (echoed.is_none() || echoed == Some(&challenge)).then_some(status)
    })
    .await
}

/// Parses a status response: an info string, then a line per player of the form
/// `score ping "name"`.
fn parse_status(s: &str) -> Status {
    let mut lines = s.lines();
    let info = parse_info(lines.next().unwrap_or_default());
    let players = lines
        .filter_map(|line| {
            let mut parts = line.splitn(3, ' ');
            let score = parts.next()?.parse().ok()?;
            let ping = parts.next()?.parse().ok()?;
            let name = parts.next()?.trim_matches(['"', '\0']).to_owned();
            Some(Player { name, score, ping })
        })
        .collect();
    Status { info, players }
}

/// Sends an out-of-band command and returns the first reply parse accepts.
async fn query<T>(
    addr: SocketAddr,
    wait: Duration,
    command: &str,
    parse: impl Fn(&str) -> Option<T>,
) -> std::io::Result<T> {
    let bind: SocketAddr = if addr.is_ipv4() {
        ([0, 0, 0, 0], 0).into()
    } else {
        ([0u16; 8], 0).into()
    };
    let socket = UdpSocket::bind(bind).await?;
    socket.connect(addr).await?;
    socket.send(&oob(command.as_bytes())).await?;

    // Status responses carry a line per player, so they can be larger than info responses.
    let mut buf = [0u8; 16384];
    let deadline = tokio::time::Instant::now() + wait;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let len = timeout(remaining, socket.recv(&mut buf))
            .await
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::TimedOut))??;
        // Player names can have bytes that aren't UTF-8.
        let text = buf[..len]
            .strip_prefix(OOB_PREFIX)
            .map(String::from_utf8_lossy);
        if let Some(reply) = text.as_deref().and_then(&parse) {
            return Ok(reply);
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
    fn parses_status_response() {
        let status = parse_status(
            "\\mapname\\q3dm17\\sv_maxclients\\8\n12 48 \"^1Ranger\"\n3 0 \"Sarge\"\nbad line\n",
        );
        assert_eq!(status.info["mapname"], "q3dm17");
        assert_eq!(
            status.players,
            vec![
                Player {
                    name: "^1Ranger".to_owned(),
                    score: 12,
                    ping: 48,
                },
                Player {
                    name: "Sarge".to_owned(),
                    score: 3,
                    ping: 0,
                },
            ]
        );
        assert!(parse_status("\\mapname\\q3dm1\n").players.is_empty());
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
