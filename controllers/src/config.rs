use crate::prelude::*;
use crate::quake_server::{GameType, GameTypeName, QuakeServerSpec};
use anyhow::Context as _;
use std::fmt;

/// Image used for game server pods unless overridden by the QUAKE_SERVER_IMAGE environment
/// variable.
pub const DEFAULT_SERVER_IMAGE: &str = "ghcr.io/chrisrx/quake-kube-server:latest";

/// Controller-wide settings, read from the YAML file named by QUAKE_KUBE_CONFIG.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default)]
    pub game_servers: GameServersConfig,
    #[serde(default)]
    pub master: MasterConfig,
    #[serde(default)]
    pub gateway: GatewayConfig,
}

/// Settings for the gateway, which serves the web app and proxies browser clients.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GatewayConfig {
    /// HTTP port the gateway listens on.
    #[serde(default = "GatewayConfig::default_port")]
    pub port: u16,
    /// Namespace for servers created from the web app.
    #[serde(default = "GatewayConfig::default_namespace")]
    pub namespace: String,
    /// How many servers can be created from the web app at once. 0 turns creating servers off.
    #[serde(default = "GatewayConfig::default_max_servers")]
    pub max_servers: usize,
    /// How many of those each person can have at once, when the gateway knows who people are.
    #[serde(default)]
    pub max_servers_per_user: Option<usize>,
    /// Servers created from the web app are deleted after nobody has played on them for this
    /// long, e.g. "30m". Empty or "0" keeps them until someone deletes them.
    #[serde(
        default = "GatewayConfig::default_idle_timeout",
        deserialize_with = "deserialize_duration"
    )]
    pub idle_timeout: Option<Duration>,
    /// The games the web app offers when creating a server.
    #[serde(default = "GatewayConfig::default_presets")]
    pub presets: Vec<Preset>,
    #[serde(default)]
    pub webtransport: WebTransportConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    /// Directory with the ioquake3 web client's ioquake3.js and ioquake3.wasm.
    #[serde(default = "GatewayConfig::default_client_dir")]
    pub client_dir: std::path::PathBuf,
}

/// WebTransport carries browser clients' game traffic in unreliable datagrams, like native
/// clients' UDP. Browsers fall back to a WebSocket when it's unavailable.
#[derive(Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct WebTransportConfig {
    pub enabled: bool,
    /// UDP port the gateway listens on.
    pub port: u16,
    /// Address browsers connect to, as host:port. When unset, browsers use the web app's
    /// hostname with public_port.
    pub public_address: Option<String>,
    pub public_port: u16,
    /// Directory with a tls.crt and tls.key that browsers trust. When unset, the gateway uses a
    /// self-signed certificate, which browsers accept by its hash.
    pub tls_dir: Option<std::path::PathBuf>,
}

/// How the gateway knows who people are. It doesn't sign them in itself: a proxy in front of it
/// does, and passes on who they are with each request.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct AuthConfig {
    pub mode: AuthMode,
    pub iap: IapConfig,
    pub header: HeaderAuthConfig,
    /// File with the key that signs browsers' WebTransport tickets. When unset, each gateway
    /// replica makes its own, so with several replicas a ticket may not work on the replica a
    /// session reaches.
    pub ticket_key_file: Option<std::path::PathBuf>,
}

#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    /// Anyone who reaches the web app can use it, and servers have no owners.
    #[default]
    None,
    /// Google Cloud's Identity-Aware Proxy, whose signed assertion is checked.
    Iap,
    /// Another proxy that signs people in, e.g. oauth2-proxy, which names them in a header. The
    /// gateway must only be reachable through it.
    Header,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct IapConfig {
    /// The audience IAP signs its assertions for, e.g.
    /// /projects/123/global/backendServices/456.
    pub audience: Option<String>,
    /// Accepts any audience in this project when the audience isn't known in advance, e.g.
    /// because GKE names the backend service.
    pub project_number: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct HeaderAuthConfig {
    /// Header with the user's email address.
    pub user: String,
    /// Header with the user's groups, separated by commas.
    pub groups: Option<String>,
}

impl Default for HeaderAuthConfig {
    fn default() -> Self {
        HeaderAuthConfig {
            user: "X-Forwarded-Email".to_owned(),
            groups: None,
        }
    }
}

impl Default for WebTransportConfig {
    fn default() -> Self {
        WebTransportConfig {
            enabled: false,
            port: 4433,
            public_address: None,
            public_port: 443,
            tls_dir: None,
        }
    }
}

impl GatewayConfig {
    fn default_port() -> u16 {
        8080
    }

    fn default_namespace() -> String {
        "default".to_owned()
    }

    fn default_max_servers() -> usize {
        10
    }

    fn default_idle_timeout() -> Option<Duration> {
        Some(Duration::from_secs(30 * 60))
    }

    fn default_client_dir() -> std::path::PathBuf {
        "/usr/share/quake-kube/client".into()
    }

    /// The Quake 3 demo, which is all the server image has.
    fn default_presets() -> Vec<Preset> {
        let game_type = |name: &str, type_| PresetGameType {
            name: name.to_owned(),
            type_: Some(GameType::Named(type_)),
            cvars: BTreeMap::new(),
        };
        vec![Preset {
            name: "quake3".to_owned(),
            title: "Quake 3".to_owned(),
            game_types: vec![
                game_type("Free For All", GameTypeName::FreeForAll),
                game_type("Tournament", GameTypeName::Tournament),
                game_type("Team Deathmatch", GameTypeName::TeamDeathmatch),
                game_type("Capture The Flag", GameTypeName::CaptureTheFlag),
            ],
            maps: ["q3dm1", "q3dm7", "q3dm17", "q3tourney2"]
                .map(String::from)
                .to_vec(),
            bots: true,
            spec: QuakeServerSpec::default(),
        }]
    }
}

/// A game the web app can create servers for, e.g. a mod and where its content comes from.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    /// Identifies the preset, e.g. "urban-terror".
    pub name: String,
    /// Shown in the web app, e.g. "Urban Terror 4.3".
    pub title: String,
    pub game_types: Vec<PresetGameType>,
    /// Maps to choose from. They must be in the preset's content.
    pub maps: Vec<String>,
    /// Whether the game has bots.
    #[serde(default = "Preset::default_bots")]
    pub bots: bool,
    /// The start of each server's spec, e.g. game.fsGame and content. The web app fills in the
    /// name, game type, map, bots and password.
    #[serde(default)]
    pub spec: QuakeServerSpec,
}

impl Preset {
    fn default_bots() -> bool {
        true
    }
}

/// A game type offered by a preset. Mods choose theirs differently, by g_gametype like Urban
/// Terror, or by a cvar like CPMA's mode_start.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PresetGameType {
    /// Shown in the web app, e.g. "Capture The Flag".
    pub name: String,
    #[serde(rename = "type")]
    pub type_: Option<GameType>,
    #[serde(default)]
    pub cvars: BTreeMap<String, String>,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        GatewayConfig {
            port: Self::default_port(),
            namespace: Self::default_namespace(),
            max_servers: Self::default_max_servers(),
            max_servers_per_user: None,
            idle_timeout: Self::default_idle_timeout(),
            presets: Self::default_presets(),
            webtransport: WebTransportConfig::default(),
            auth: AuthConfig::default(),
            client_dir: Self::default_client_dir(),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MasterConfig {
    /// UDP port the master server listens on.
    #[serde(default = "MasterConfig::default_port")]
    pub port: u16,
    /// Address game servers send heartbeats to, as host:port. Usually the master's in-cluster
    /// Service. Servers don't send heartbeats when unset.
    pub address: Option<String>,
}

impl MasterConfig {
    fn default_port() -> u16 {
        27950
    }
}

impl Default for MasterConfig {
    fn default() -> Self {
        MasterConfig {
            port: Self::default_port(),
            address: None,
        }
    }
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct GameServersConfig {
    /// Image for game server pods. This comes from QUAKE_SERVER_IMAGE rather than the file so
    /// Tilt can substitute its own build.
    #[serde(skip)]
    pub image: String,
    #[serde(default)]
    pub exposure: Exposure,
}

/// How game servers are made reachable by native Quake 3 clients.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Exposure {
    #[serde(rename = "type", default)]
    pub type_: ExposureType,
    /// Annotations added to each game server's external Service, e.g. for a cloud load balancer
    /// or a shared MetalLB address.
    #[serde(default)]
    pub annotations: BTreeMap<String, String>,
    /// External ports are allocated from this range when set. Otherwise every server uses
    /// 27960, or a NodePort chosen by Kubernetes.
    pub port_range: Option<PortRange>,
}

#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExposureType {
    #[default]
    LoadBalancer,
    NodePort,
    HostPort,
    /// Only reachable from inside the cluster, e.g. by browser clients through the gateway.
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortRange {
    pub start: i32,
    pub end: i32,
}

impl PortRange {
    pub fn contains(&self, port: i32) -> bool {
        (self.start..=self.end).contains(&port)
    }
}

impl fmt::Display for PortRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}

impl<'de> Deserialize<'de> for PortRange {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let parsed = s.split_once('-').and_then(|(start, end)| {
            Some(PortRange {
                start: start.trim().parse().ok()?,
                end: end.trim().parse().ok()?,
            })
        });
        match parsed {
            Some(range) if 0 < range.start && range.start <= range.end && range.end <= 65535 => {
                Ok(range)
            }
            _ => Err(serde::de::Error::custom(format!(
                "invalid port range {s:?}, expected START-END"
            ))),
        }
    }
}

/// Parses a duration of hours, minutes and seconds, e.g. "1h30m" or "45s". Zero is None.
fn parse_duration(s: &str) -> Option<Option<Duration>> {
    let s = s.trim();
    if s.is_empty() || s == "0" {
        return Some(None);
    }
    let mut secs = 0u64;
    let mut number = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            number.push(c);
            continue;
        }
        let unit = match c {
            'h' => 3600,
            'm' => 60,
            's' => 1,
            _ => return None,
        };
        secs += number.parse::<u64>().ok()? * unit;
        number.clear();
    }
    if !number.is_empty() {
        return None;
    }
    Some((secs > 0).then(|| Duration::from_secs(secs)))
}

fn deserialize_duration<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Duration>, D::Error> {
    let s = Option::<String>::deserialize(deserializer)?.unwrap_or_default();
    parse_duration(&s).ok_or_else(|| {
        serde::de::Error::custom(format!("invalid duration {s:?}, expected e.g. 30m or 1h"))
    })
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let mut config: Config = match std::env::var("QUAKE_KUBE_CONFIG") {
            Ok(path) => {
                let data = std::fs::read_to_string(&path)
                    .with_context(|| format!("reading config {path}"))?;
                serde_yaml_ng::from_str(&data).with_context(|| format!("parsing config {path}"))?
            }
            Err(_) => Config::default(),
        };
        config.game_servers.image =
            std::env::var("QUAKE_SERVER_IMAGE").unwrap_or_else(|_| DEFAULT_SERVER_IMAGE.to_owned());
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_presets() {
        let config: GatewayConfig = serde_yaml_ng::from_str(
            r#"
presets:
  - name: urban-terror
    title: Urban Terror 4.3
    bots: false
    gameTypes:
      - {name: Capture The Flag, type: 7}
    maps: [ut4_casa]
    spec:
      game: {fsGame: q3ut4}
      content: [{image: example.com/urt:4.3.4}]
  - name: cpma
    title: CPMA
    gameTypes:
      - {name: Duel, cvars: {mode_start: 1v1}}
    maps: [cpm22]
"#,
        )
        .unwrap();
        let [urt, cpma] = &config.presets[..] else {
            panic!("expected two presets");
        };
        assert!(!urt.bots);
        assert_eq!(urt.game_types[0].type_, Some(GameType::Number(7)));
        assert_eq!(
            urt.spec.game.as_ref().unwrap().fs_game.as_deref(),
            Some("q3ut4")
        );
        assert!(cpma.bots);
        assert_eq!(cpma.game_types[0].type_, None);
        assert_eq!(cpma.game_types[0].cvars["mode_start"], "1v1");
        // Without presets, the web app offers the Quake 3 demo.
        assert_eq!(GatewayConfig::default().presets[0].name, "quake3");
    }

    #[test]
    fn parses_durations() {
        let minutes = |m: u64| Some(Some(Duration::from_secs(m * 60)));
        assert_eq!(parse_duration("30m"), minutes(30));
        assert_eq!(parse_duration("1h30m"), minutes(90));
        assert_eq!(parse_duration("45s"), Some(Some(Duration::from_secs(45))));
        assert_eq!(parse_duration(""), Some(None));
        assert_eq!(parse_duration("0"), Some(None));
        assert_eq!(parse_duration("0m"), Some(None));
        assert_eq!(parse_duration("30"), None);
        assert_eq!(parse_duration("1d"), None);
        let config: GatewayConfig = serde_yaml_ng::from_str("idleTimeout: 2h").unwrap();
        assert_eq!(config.idle_timeout, minutes(120).unwrap());
        let config: GatewayConfig = serde_yaml_ng::from_str("idleTimeout: null").unwrap();
        assert_eq!(config.idle_timeout, None);
        let config: GatewayConfig = serde_yaml_ng::from_str("port: 80").unwrap();
        assert_eq!(config.idle_timeout, minutes(30).unwrap());
    }
}
