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
    /// The games the web app offers when creating a server.
    #[serde(default = "GatewayConfig::default_presets")]
    pub presets: Vec<Preset>,
    #[serde(default)]
    pub webtransport: WebTransportConfig,
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
            presets: Self::default_presets(),
            webtransport: WebTransportConfig::default(),
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
}
