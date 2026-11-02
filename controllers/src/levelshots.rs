//! Thumbnails of the levelshots of every map the web app offers, so its menus show pictures
//! of maps for games no server is running. For each preset, a pod assembles the preset's
//! content like a server's pod and serves the thumbnails, which are kept in a ConfigMap until
//! the preset's content or maps change.

use crate::config::Preset;
use crate::prelude::*;
use crate::quake_server::{checksum, pod_problem, HTTP_PORT};
use axum::body::Bytes;
use futures::stream;
use kube::api::PostParams;
use std::collections::HashMap;
use std::sync::RwLock;
use tokio::time::Instant;
use tracing::*;

const FIELD_MANAGER: &str = "quake-kube-gateway";

/// Annotation on a ConfigMap of thumbnails with the checksum of what they were made from.
const SOURCE_ANNOTATION: &str = "quake.kube/levelshots-source";

/// How long a pod has to assemble a preset's content, which can mean pulling a large image.
const POD_TIMEOUT: Duration = Duration::from_secs(600);

/// How long until presets whose thumbnails failed are tried again.
const RETRY_INTERVAL: Duration = Duration::from_secs(300);

/// How many presets' pods run at once.
const CONCURRENCY: usize = 2;

/// The start of the name of each preset's ConfigMap of thumbnails.
const CONFIGMAP_PREFIX: &str = "quake-kube-levelshots-";

pub struct Levelshots {
    client: Client,
    namespace: String,
    /// The game server image, which has q3.
    image: String,
    http: reqwest::Client,
    /// Each preset's maps, by preset.
    cache: RwLock<HashMap<String, PresetMaps>>,
}

/// What's known about a preset's maps.
#[derive(Default)]
struct PresetMaps {
    /// Thumbnails, by lowercase map name.
    shots: HashMap<String, Bytes>,
    /// What maps' arena files say about them, by lowercase map name.
    arenas: BTreeMap<String, Arena>,
}

/// What a map's arena file says about it.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Arena {
    /// Its long name, e.g. "The Longest Yard".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The game types it's made for, e.g. ffa, tourney, team or ctf.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<String>,
}

#[derive(Deserialize)]
struct Response {
    /// JPEGs, base64 encoded.
    maps: BTreeMap<String, ByteString>,
    #[serde(default)]
    arenas: BTreeMap<String, Arena>,
}

/// The key of a ConfigMap's arena details.
const ARENAS_KEY: &str = "arenas.json";

/// Changes when what's kept in the ConfigMaps does, so they're made again.
const FORMAT: u32 = 2;

impl Levelshots {
    pub fn new(client: Client, namespace: String, image: String) -> Arc<Self> {
        Arc::new(Levelshots {
            client,
            namespace,
            image,
            http: reqwest::Client::new(),
            cache: RwLock::default(),
        })
    }

    /// A map's thumbnail, once the preset's are ready.
    pub fn get(&self, preset: &str, map: &str) -> Option<Bytes> {
        let cache = self.cache.read().unwrap();
        cache.get(preset)?.shots.get(&map.to_lowercase()).cloned()
    }

    /// Whether the preset's thumbnails are ready.
    pub fn ready(&self, preset: &str) -> bool {
        self.cache.read().unwrap().contains_key(preset)
    }

    /// A map's long name from its arena file, e.g. "The Longest Yard", once known.
    pub fn title(&self, preset: &str, map: &str) -> Option<String> {
        let cache = self.cache.read().unwrap();
        let title = cache
            .get(preset)?
            .arenas
            .get(&map.to_lowercase())?
            .title
            .clone()?;
        // Some arena files just repeat the map's name.
        (!title.eq_ignore_ascii_case(map)).then_some(title)
    }

    /// What the preset's maps' arena files say about them, by lowercase map name, once
    /// they're known.
    pub fn arenas(&self, preset: &str) -> BTreeMap<String, Arena> {
        let cache = self.cache.read().unwrap();
        cache
            .get(preset)
            .map(|maps| maps.arenas.clone())
            .unwrap_or_default()
    }

    /// Makes the presets' thumbnails, trying again later for any that fail, after deleting
    /// those of presets that are gone.
    pub async fn run(self: Arc<Self>, presets: Vec<Preset>) {
        if let Err(error) = self.delete_stale(&presets).await {
            warn!("deleting old levelshots failed: {error}");
        }
        let mut pending = presets;
        while !pending.is_empty() {
            let results: Vec<_> = stream::iter(pending)
                .map(|preset| {
                    let this = self.clone();
                    async move {
                        let result = this.ensure(&preset).await;
                        (preset, result)
                    }
                })
                .buffer_unordered(CONCURRENCY)
                .collect()
                .await;
            pending = results
                .into_iter()
                .filter_map(|(preset, result)| match result {
                    Ok(()) => None,
                    Err(error) => {
                        warn!(preset = %preset.name, "making levelshots failed: {error:#}");
                        Some(preset)
                    }
                })
                .collect();
            if !pending.is_empty() {
                tokio::time::sleep(RETRY_INTERVAL).await;
            }
        }
    }

    /// Deletes the ConfigMaps of thumbnails for presets that are no longer offered.
    async fn delete_stale(&self, presets: &[Preset]) -> anyhow::Result<()> {
        let configmaps = Api::<ConfigMap>::namespaced(self.client.clone(), &self.namespace);
        let managed =
            ListParams::default().labels(&format!("app.kubernetes.io/managed-by={FIELD_MANAGER}"));
        for cm in configmaps.list(&managed).await? {
            let name = cm.name_any();
            let Some(preset) = name.strip_prefix(CONFIGMAP_PREFIX) else {
                continue;
            };
            if !presets.iter().any(|p| p.name == preset) {
                info!(%preset, "deleting levelshots of a preset that's gone");
                configmaps.delete(&name, &DeleteParams::default()).await?;
            }
        }
        Ok(())
    }

    /// Loads a preset's thumbnails, making them first if they're missing or out of date.
    async fn ensure(&self, preset: &Preset) -> anyhow::Result<()> {
        let source = source_checksum(preset);
        let name = format!("{CONFIGMAP_PREFIX}{}", preset.name);
        let configmaps = Api::<ConfigMap>::namespaced(self.client.clone(), &self.namespace);
        let existing = configmaps.get_opt(&name).await?;
        let current = existing.filter(|cm| {
            cm.annotations().get(SOURCE_ANNOTATION).map(String::as_str) == Some(source.as_str())
        });
        let maps = match current {
            Some(cm) => PresetMaps {
                arenas: (cm.data.as_ref())
                    .and_then(|data| serde_json::from_str(data.get(ARENAS_KEY)?).ok())
                    .unwrap_or_default(),
                shots: (cm.binary_data.unwrap_or_default().into_iter())
                    .map(|(file, data)| {
                        (
                            file.trim_end_matches(".jpg").to_owned(),
                            Bytes::from(data.0),
                        )
                    })
                    .collect(),
            },
            None => {
                let maps = self.make(preset, &source).await?;
                let binary_data: BTreeMap<String, ByteString> = (maps.shots.iter())
                    .filter(|(map, _)| valid_key(map))
                    .map(|(map, data)| (format!("{map}.jpg"), ByteString(data.to_vec())))
                    .collect();
                let configmap = ConfigMap {
                    metadata: ObjectMeta {
                        name: Some(name.clone()),
                        namespace: Some(self.namespace.clone()),
                        annotations: Some(BTreeMap::from([(
                            SOURCE_ANNOTATION.to_owned(),
                            source.clone(),
                        )])),
                        labels: Some(BTreeMap::from([(
                            "app.kubernetes.io/managed-by".to_owned(),
                            FIELD_MANAGER.to_owned(),
                        )])),
                        ..ObjectMeta::default()
                    },
                    binary_data: Some(binary_data),
                    data: Some(BTreeMap::from([(
                        ARENAS_KEY.to_owned(),
                        serde_json::to_string(&maps.arenas)?,
                    )])),
                    ..ConfigMap::default()
                };
                configmaps
                    .patch(
                        &name,
                        &PatchParams::apply(FIELD_MANAGER).force(),
                        &Patch::Apply(&configmap),
                    )
                    .await?;
                info!(
                    preset = %preset.name,
                    count = maps.shots.len(),
                    arenas = maps.arenas.len(),
                    "made levelshots"
                );
                maps
            }
        };
        self.cache
            .write()
            .unwrap()
            .insert(preset.name.clone(), maps);
        Ok(())
    }

    /// Runs a pod with the preset's content and fetches the thumbnails it serves.
    async fn make(&self, preset: &Preset, source: &str) -> anyhow::Result<PresetMaps> {
        let pods = Api::<Pod>::namespaced(self.client.clone(), &self.namespace);
        // Named for what it's made from, so gateway replicas share one pod.
        let name = format!("quake-kube-levelshots-{}-{}", preset.name, &source[..8]);
        let mut server = QuakeServer::new(&name, preset.spec.clone());
        server.metadata.namespace = Some(self.namespace.clone());
        let pod = server.levelshots_pod(&self.image, &preset.maps)?;
        match pods.create(&PostParams::default(), &pod).await {
            Ok(_) => info!(preset = %preset.name, pod = %name, "making levelshots"),
            Err(kube::Error::Api(status)) if status.code == 409 => {}
            Err(error) => return Err(error.into()),
        }
        let result = self.fetch(&pods, &name).await;
        if let Err(error) = pods.delete(&name, &DeleteParams::background()).await {
            debug!(pod = %name, "deleting levelshots pod failed: {error}");
        }
        result
    }

    /// Waits for the pod to serve the thumbnails and fetches them.
    async fn fetch(&self, pods: &Api<Pod>, name: &str) -> anyhow::Result<PresetMaps> {
        let deadline = Instant::now() + POD_TIMEOUT;
        let ip = loop {
            let pod = pods.get(name).await?;
            let status = pod.status.as_ref();
            if let Some(problem) = pod_problem(&pod) {
                anyhow::bail!("{problem}");
            }
            if status.and_then(|s| s.phase.as_deref()) == Some("Failed") {
                anyhow::bail!("the pod failed");
            }
            let ready = (status
                .and_then(|s| s.conditions.as_ref())
                .into_iter()
                .flatten())
            .any(|c| c.type_ == "Ready" && c.status == "True");
            if let (true, Some(ip)) = (ready, status.and_then(|s| s.pod_ip.clone())) {
                break ip;
            }
            if Instant::now() > deadline {
                anyhow::bail!("the pod wasn't ready in time");
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        };
        let host = if ip.contains(':') {
            format!("[{ip}]")
        } else {
            ip
        };
        let response: Response = self
            .http
            .get(format!("http://{host}:{HTTP_PORT}/levelshots"))
            .timeout(Duration::from_secs(30))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(PresetMaps {
            shots: (response.maps.into_iter())
                .map(|(map, data)| (map, Bytes::from(data.0)))
                .collect(),
            arenas: response.arenas,
        })
    }
}

/// A checksum of what a preset's thumbnails are made from.
fn source_checksum(preset: &Preset) -> String {
    let game = preset.spec.game.as_ref();
    checksum(&(
        FORMAT,
        game.and_then(|game| game.fs_game.as_ref()),
        &preset.spec.content,
        &preset.maps,
    ))
}

/// Whether a map's name can be a ConfigMap key. Map names can have a +, which keys can't.
fn valid_key(map: &str) -> bool {
    map.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_changes_with_maps_and_content() {
        let preset =
            |value: serde_json::Value| -> Preset { serde_json::from_value(value).unwrap() };
        let base =
            serde_json::json!({ "name": "q", "title": "Q", "gameTypes": [], "maps": ["q3dm1"] });
        let a = source_checksum(&preset(base.clone()));
        let mut more = base.clone();
        more["maps"] = serde_json::json!(["q3dm1", "q3dm7"]);
        assert_ne!(a, source_checksum(&preset(more)));
        let mut content = base.clone();
        content["spec"] =
            serde_json::json!({ "content": [{ "url": "https://example.com/a.pk3" }] });
        assert_ne!(a, source_checksum(&preset(content)));
        // The title and game types don't change the pictures.
        let mut title = base;
        title["title"] = serde_json::json!("Quake");
        assert_eq!(a, source_checksum(&preset(title)));
        assert!(valid_key("ut4_abbey"));
        assert!(!valid_key("q3dm1+"));
    }
}
