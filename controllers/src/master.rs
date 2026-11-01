//! A Quake 3 master server that lists the QuakeServers managed by this controller.
//!
//! Servers are taken from the controller's cache rather than registered by heartbeats, so each
//! one is listed with its external address instead of the in-cluster address its heartbeats
//! come from. Heartbeats only trigger an early refresh.

use crate::protocol;
use crate::registry::{Entry, Registry};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tracing::*;

/// Game name sent by clients and servers for Quake 3.
const GAME_NAME: &str = "Quake3Arena";

/// Answers server list requests and heartbeats.
pub async fn serve(socket: UdpSocket, registry: Arc<Registry>) -> std::io::Result<()> {
    let mut buf = [0u8; 1024];
    loop {
        let (len, src) = socket.recv_from(&mut buf).await?;
        let Some(command) = protocol::parse_oob(&buf[..len]) else {
            continue;
        };
        let mut args = command.split_ascii_whitespace();
        match args.next() {
            Some("heartbeat") => {
                debug!(%src, "heartbeat");
                registry.refresh();
            }
            Some(cmd @ ("getservers" | "getserversExt")) => {
                let extended = cmd == "getserversExt";
                let Some(filter) = Filter::parse(args, extended) else {
                    continue;
                };
                let servers: Vec<SocketAddr> = registry
                    .entries()
                    .iter()
                    .filter(|entry| filter.matches(entry))
                    .filter_map(|entry| entry.address)
                    .collect();
                debug!(%src, ?filter, count = servers.len(), "sending server list");
                for packet in protocol::servers_response(&servers, extended) {
                    if let Err(error) = socket.send_to(&packet, src).await {
                        warn!(%src, "sending server list failed: {error}");
                    }
                }
            }
            _ => {}
        }
    }
}

/// Options of a getservers or getserversExt request.
#[derive(Debug, Default, PartialEq)]
struct Filter {
    empty: bool,
    full: bool,
    gametype: Option<i32>,
    ipv4: bool,
    ipv6: bool,
}

impl Filter {
    /// Parses the arguments after the command. Returns None for requests about other games.
    fn parse<'a>(args: impl Iterator<Item = &'a str>, extended: bool) -> Option<Self> {
        let mut args = args.peekable();
        // ioq3 leaves out the game name when asking for Quake 3 servers with getservers.
        if (extended || args.peek()?.parse::<i32>().is_err())
            && !args.next()?.eq_ignore_ascii_case(GAME_NAME)
        {
            return None;
        }
        // The protocol version isn't checked: ioq3 servers accept both 68 and 71.
        args.next()?.parse::<i32>().ok()?;

        let mut filter = Filter::default();
        for arg in args {
            match arg {
                "empty" => filter.empty = true,
                "full" => filter.full = true,
                "ipv4" => filter.ipv4 = true,
                "ipv6" => filter.ipv6 = true,
                "ffa" => filter.gametype = Some(0),
                "tourney" => filter.gametype = Some(1),
                "team" => filter.gametype = Some(3),
                "ctf" => filter.gametype = Some(4),
                _ => {
                    if let Some(gametype) = arg.strip_prefix("gametype=") {
                        filter.gametype = gametype.parse().ok();
                    }
                }
            }
        }
        Some(filter)
    }

    fn matches(&self, entry: &Entry) -> bool {
        let get = |key: &str| entry.info.get(key).and_then(|v| v.parse::<i32>().ok());
        let clients = get("clients").unwrap_or(0);
        if !self.empty && clients == 0 {
            return false;
        }
        if !self.full && get("sv_maxclients").is_some_and(|max| clients >= max) {
            return false;
        }
        if self
            .gametype
            .is_some_and(|gametype| get("gametype") != Some(gametype))
        {
            return false;
        }
        // Without ipv4 or ipv6, both are wanted.
        match entry.address {
            Some(SocketAddr::V4(_)) => self.ipv4 || !self.ipv6,
            Some(SocketAddr::V6(_)) => self.ipv6 || !self.ipv4,
            // Native clients can't reach it.
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tokio::time::Instant;

    fn entry(address: &str, clients: i32, max: i32, gametype: i32) -> Entry {
        Entry {
            address: Some(address.parse().unwrap()),
            info: BTreeMap::from([
                ("clients".to_owned(), clients.to_string()),
                ("sv_maxclients".to_owned(), max.to_string()),
                ("gametype".to_owned(), gametype.to_string()),
            ]),
            updated: Instant::now(),
        }
    }

    fn parse(request: &str) -> Option<Filter> {
        let mut args = request.split_ascii_whitespace();
        let extended = args.next() == Some("getserversExt");
        Filter::parse(args, extended)
    }

    #[test]
    fn parses_requests() {
        assert_eq!(parse("getservers 68"), Some(Filter::default()));
        assert_eq!(
            parse("getservers 68 empty full"),
            Some(Filter {
                empty: true,
                full: true,
                ..Filter::default()
            })
        );
        assert_eq!(
            parse("getservers Quake3Arena 71 ctf"),
            Some(Filter {
                gametype: Some(4),
                ..Filter::default()
            })
        );
        assert_eq!(
            parse("getserversExt Quake3Arena 71 ipv6 gametype=3"),
            Some(Filter {
                ipv6: true,
                gametype: Some(3),
                ..Filter::default()
            })
        );
        assert_eq!(parse("getservers OpenArena 71"), None);
        assert_eq!(parse("getserversExt 68"), None);
        assert_eq!(parse("getservers"), None);
    }

    #[test]
    fn filters_servers() {
        let default = parse("getservers 68").unwrap();
        assert!(default.matches(&entry("10.0.0.1:27960", 3, 12, 0)));
        assert!(!default.matches(&entry("10.0.0.1:27960", 0, 12, 0)));
        assert!(!default.matches(&entry("10.0.0.1:27960", 12, 12, 0)));

        let all = parse("getservers 68 empty full").unwrap();
        assert!(all.matches(&entry("10.0.0.1:27960", 0, 12, 0)));
        assert!(all.matches(&entry("10.0.0.1:27960", 12, 12, 0)));
        // Servers without an external address yet can't be listed.
        let unreachable = Entry {
            address: None,
            ..entry("10.0.0.1:27960", 3, 12, 0)
        };
        assert!(!all.matches(&unreachable));

        let ctf = parse("getservers 68 empty ctf").unwrap();
        assert!(ctf.matches(&entry("10.0.0.1:27960", 0, 12, 4)));
        assert!(!ctf.matches(&entry("10.0.0.1:27960", 0, 12, 0)));

        let v6 = parse("getserversExt Quake3Arena 71 empty ipv6").unwrap();
        assert!(v6.matches(&entry("[fd00::1]:27960", 0, 12, 0)));
        assert!(!v6.matches(&entry("10.0.0.1:27960", 0, 12, 0)));
    }
}
