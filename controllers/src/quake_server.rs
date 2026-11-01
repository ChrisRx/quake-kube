use crate::config::ExposureType;
use crate::prelude::*;
use crate::quake_server_controller::Context;
use k8s_openapi::NamespaceResourceScope;
use rand::distr::{Alphanumeric, SampleString};
use schemars::{json_schema, Schema, SchemaGenerator};
use serde::de::DeserializeOwned;
use serde_json::json;
use std::borrow::Cow;
use tracing::*;

const FIELD_MANAGER: &str = "quake-kube-controller";

/// Port the game server listens on inside the pod.
pub const GAME_PORT: i32 = 27960;

/// Port of the server info, content and health checks inside the pod.
pub const HTTP_PORT: i32 = 8080;

/// Where content images are mounted in the server's pod.
const CONTENT_IMAGES_DIR: &str = "/content-images";

/// Annotation on the server's pod with a checksum of what it downloads, so the pod is
/// replaced when that changes.
const CONTENT_CHECKSUM_ANNOTATION: &str = "quake.kube/content-checksum";

/// A Quake 3 dedicated server. Every field is optional and falls back to the server's defaults.
#[derive(CustomResource, Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[kube(group = "quake.kube", version = "v1", kind = "QuakeServer")]
#[kube(shortname = "qs", namespaced, status = "QuakeServerStatus")]
#[kube(doc = "A Quake 3 dedicated server managed by quake-kube.")]
#[kube(printcolumn(name = "Ready", type_ = "boolean", json_path = ".status.ready"))]
#[kube(printcolumn(name = "Address", type_ = "string", json_path = ".status.address"))]
#[kube(printcolumn(
    name = "Age",
    type_ = "date",
    json_path = ".metadata.creationTimestamp"
))]
#[serde(rename_all = "camelCase")]
pub struct QuakeServerSpec {
    /// Frag limit for maps that don't set their own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frag_limit: Option<i32>,
    /// Time limit for maps that don't set their own, e.g. "15m".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bot: Option<BotSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game: Option<GameSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<ServerSpec>,
    /// Console commands run once the server starts, e.g. "addbot sarge 2".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<String>,
    /// Map rotation, played in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub maps: Vec<MapSpec>,
    /// Cvars set before the first map loads, e.g. a mod's settings like CPMA's mode_start.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cvars: BTreeMap<String, String>,
    /// Files downloaded into the server's content before it starts, such as a mod and its
    /// maps. Browser clients download the server's content from it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<ContentSpec>,
}

/// Where some of the server's content comes from: a file to download, or a content image.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-kubernetes-validations" = [{
    "rule": "has(self.url) != has(self.image)",
    "message": "set one of url and image",
}]))]
pub struct ContentSpec {
    /// A pk3, or a zip archive of game directories, e.g. a mod's release.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// An OCI image whose top-level directories are game directories, e.g. q3ut4/. It's
    /// mounted in the pod and its files linked into the content, so nothing is downloaded
    /// when the node already has the image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// SHA-256 checksum of the download in hex. The server doesn't start if it doesn't match.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(regex(pattern = r"^[0-9a-fA-F]{64}$"))]
    pub sha256: Option<String>,
    /// Directory the files go in, e.g. "cpma". A pk3 goes in game.fsGame, or baseq3, by
    /// default. An archive or image goes at the top, where each game has its directory, and
    /// an archive's files outside a directory are skipped.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(regex(
        pattern = r"^[A-Za-z0-9_-][A-Za-z0-9_.-]*(/[A-Za-z0-9_-][A-Za-z0-9_.-]*)*$"
    ))]
    pub dir: Option<String>,
    /// Leading directories removed from the paths in an archive, like tar's
    /// --strip-components.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strip_components: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BotSpec {
    /// Bots are added until there are at least this many players.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_players: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_chat: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GameSpec {
    /// Game type of maps that don't set their own.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<GameType>,
    /// Mod the server runs (fs_game), e.g. "cpma" or "q3ut4". Its files come from content.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(regex(pattern = r"^[A-Za-z0-9_-][A-Za-z0-9_.-]*$"))]
    pub fs_game: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_respawn: Option<bool>,
    /// Idle players are kicked after this long, e.g. "10m".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inactivity: Option<String>,
    /// Message of the day shown to players when they join.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motd: Option<String>,
    /// Password players need to join.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quad_factor: Option<i32>,
    /// Weapon respawn time in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon_respawn: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub single_player_skill: Option<i32>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServerSpec {
    /// Name shown in server browsers. Defaults to the QuakeServer name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_clients: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_download: Option<bool>,
    #[serde(rename = "downloadURL", skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MapSpec {
    #[schemars(regex(pattern = r"^[A-Za-z0-9_.+-]+$"))]
    pub name: String,
    /// Defaults to game.type.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<GameType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frag_limit: Option<i32>,
    /// e.g. "10m"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_limit: Option<String>,
}

/// A game type by name, or by its g_gametype number for a mod's own game types, e.g. 7 for
/// Capture The Flag in Urban Terror. Mods number their game types differently, so names mean
/// the standard Quake 3 ones.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GameType {
    Named(GameTypeName),
    Number(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub enum GameTypeName {
    FreeForAll,
    Tournament,
    SinglePlayer,
    TeamDeathmatch,
    CaptureTheFlag,
}

impl JsonSchema for GameType {
    fn schema_name() -> Cow<'static, str> {
        "GameType".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "x-kubernetes-int-or-string": true,
            // Only applies to strings.
            "pattern": "^(FreeForAll|Tournament|SinglePlayer|TeamDeathmatch|CaptureTheFlag)$",
        })
    }
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuakeServerStatus {
    /// Whether the server is running. Browser clients can join it then, while native clients
    /// need address, which can take longer, e.g. for a load balancer.
    #[serde(default)]
    pub ready: bool,
    /// Address native clients connect to, as host:port.
    pub address: Option<String>,
    /// External port allocated from the controller's port range.
    pub port: Option<i32>,
    pub observed_generation: Option<i64>,
}

impl QuakeServer {
    pub async fn reconcile(&self, ctx: Arc<Context>) -> Result<Action> {
        let namespace = self
            .namespace()
            .ok_or(Error::MissingObjectKey(".metadata.namespace"))?;
        let name = self.name_any();
        let exposure = &ctx.config.game_servers.exposure;

        let port = self.allocate_port(&ctx, &namespace).await?;
        let rcon_password = self.rcon_password(&ctx.client, &namespace).await?;

        let master = ctx.config.master.address.as_deref();
        apply(
            &ctx.client,
            &namespace,
            &self.secret(master, &rcon_password)?,
        )
        .await?;
        apply(&ctx.client, &namespace, &self.internal_service()).await?;
        let external_name = format!("{name}-game");
        match exposure.type_ {
            ExposureType::LoadBalancer | ExposureType::NodePort => {
                apply(&ctx.client, &namespace, &self.external_service(&ctx, port)).await?;
            }
            ExposureType::HostPort | ExposureType::None => {
                // Remove the Service left behind if the exposure type changed.
                let services: Api<Service> = Api::namespaced(ctx.client.clone(), &namespace);
                if services.get_opt(&external_name).await?.is_some() {
                    services
                        .delete(&external_name, &DeleteParams::default())
                        .await?;
                }
            }
        }
        apply(&ctx.client, &namespace, &self.deployment(&ctx, port)).await?;

        let status = self.observe(&ctx, &namespace, port).await?;
        let exposed = status.address.is_some()
            || ctx.config.game_servers.exposure.type_ == ExposureType::None;
        let settled = status.ready && exposed;
        if self.status.as_ref() != Some(&status) {
            info!(%namespace, %name, ?status, "updating status");
            Api::<QuakeServer>::namespaced(ctx.client.clone(), &namespace)
                .patch_status(
                    &name,
                    &PatchParams::default(),
                    &Patch::Merge(json!({ "status": status })),
                )
                .await?;
            // Update the master's list now rather than at its next poll.
            ctx.registry.refresh();
        }

        // Changes to the owned Deployment and Services trigger a reconcile, but the addresses of
        // nodes and load balancers aren't watched, so poll until the server has one.
        Ok(Action::requeue(Duration::from_secs(if settled {
            300
        } else {
            5
        })))
    }

    /// Returns the external port for this server, allocating one from the configured range if
    /// it doesn't have one yet. Returns None when no range is configured.
    async fn allocate_port(&self, ctx: &Context, namespace: &str) -> Result<Option<i32>> {
        let Some(range) = ctx.config.game_servers.exposure.port_range else {
            return Ok(None);
        };
        let current = self.status.as_ref().and_then(|s| s.port);
        if let Some(port) = current.filter(|port| range.contains(*port)) {
            return Ok(Some(port));
        }

        let _guard = ctx.port_lock.lock().await;
        let used: BTreeSet<i32> = Api::<QuakeServer>::all(ctx.client.clone())
            .list(&ListParams::default())
            .await?
            .into_iter()
            .filter(|qs| qs.uid() != self.uid())
            .filter_map(|qs| qs.status?.port)
            .collect();
        let port = (range.start..=range.end)
            .find(|port| !used.contains(port))
            .ok_or(Error::PortsExhausted(range))?;

        // Record the port before anything uses it so it survives a controller restart.
        Api::<QuakeServer>::namespaced(ctx.client.clone(), namespace)
            .patch_status(
                &self.name_any(),
                &PatchParams::default(),
                &Patch::Merge(json!({ "status": { "port": port } })),
            )
            .await?;
        Ok(Some(port))
    }

    /// Returns the rcon password stored in the server's Secret, generating one for a new server.
    async fn rcon_password(&self, client: &Client, namespace: &str) -> Result<String> {
        let existing = Api::<Secret>::namespaced(client.clone(), namespace)
            .get_opt(&self.name_any())
            .await?
            .and_then(|secret| secret.data?.remove("rconPassword"))
            .map(|password| String::from_utf8_lossy(&password.0).into_owned());
        Ok(existing.unwrap_or_else(|| Alphanumeric.sample_string(&mut rand::rng(), 24)))
    }

    /// Renders the config file read by `q3 run`. Unset fields are left out so the server's
    /// defaults apply.
    fn render_config(&self, master: Option<&str>, rcon_password: &str) -> Result<String> {
        let mut spec = self.spec.clone();
        // The server plays each map with the map's own type, which otherwise defaults to
        // FreeForAll.
        if let Some(game_type) = spec.game.as_ref().and_then(|game| game.type_) {
            for map in spec.maps.iter_mut().filter(|map| map.type_.is_none()) {
                map.type_ = Some(game_type);
            }
        }
        let fs_game = spec.game.as_mut().and_then(|game| game.fs_game.take());
        let mut config = serde_json::to_value(&spec)?;
        // Tell q3 fetch where each image is mounted.
        for (i, content) in self.spec.content.iter().enumerate() {
            if content.image.is_some() {
                config["content"][i]["path"] = format!("{CONTENT_IMAGES_DIR}/{i}").into();
            }
        }
        if let Some(fs_game) = fs_game {
            config["fs"] = json!({ "game": fs_game });
        }
        let server = &mut config["server"];
        if server.get("hostname").is_none() {
            server["hostname"] = self.name_any().into();
        }
        server["password"] = rcon_password.into();
        if let Some(master) = master {
            server["listServer"] = master.into();
        }
        Ok(serde_yaml_ng::to_string(&config)?)
    }

    fn secret(&self, master: Option<&str>, rcon_password: &str) -> Result<Secret> {
        Ok(Secret {
            metadata: self.child_meta(self.name_any()),
            data: Some(BTreeMap::from([
                (
                    "config.yaml".to_owned(),
                    ByteString(self.render_config(master, rcon_password)?.into_bytes()),
                ),
                (
                    "rconPassword".to_owned(),
                    ByteString(rcon_password.as_bytes().to_vec()),
                ),
            ])),
            ..Secret::default()
        })
    }

    /// In-cluster access to the web client and game port.
    fn internal_service(&self) -> Service {
        Service {
            metadata: self.child_meta(self.name_any()),
            spec: Some(ServiceSpec {
                selector: Some(self.selector_labels()),
                ports: Some(vec![
                    ServicePort {
                        name: Some("http".to_owned()),
                        port: HTTP_PORT,
                        target_port: Some(IntOrString::String("http".to_owned())),
                        ..ServicePort::default()
                    },
                    ServicePort {
                        name: Some("game".to_owned()),
                        port: GAME_PORT,
                        protocol: Some("UDP".to_owned()),
                        target_port: Some(IntOrString::String("game".to_owned())),
                        ..ServicePort::default()
                    },
                ]),
                ..ServiceSpec::default()
            }),
            ..Service::default()
        }
    }

    /// The game port exposed outside the cluster.
    fn external_service(&self, ctx: &Context, port: Option<i32>) -> Service {
        let exposure = &ctx.config.game_servers.exposure;
        let (type_, port, node_port) = match exposure.type_ {
            ExposureType::NodePort => ("NodePort", GAME_PORT, port),
            _ => ("LoadBalancer", port.unwrap_or(GAME_PORT), None),
        };
        let mut metadata = self.child_meta(format!("{}-game", self.name_any()));
        metadata.annotations = Some(exposure.annotations.clone());
        Service {
            metadata,
            spec: Some(ServiceSpec {
                type_: Some(type_.to_owned()),
                selector: Some(self.selector_labels()),
                ports: Some(vec![ServicePort {
                    name: Some("game".to_owned()),
                    port,
                    node_port,
                    protocol: Some("UDP".to_owned()),
                    target_port: Some(IntOrString::String("game".to_owned())),
                    ..ServicePort::default()
                }]),
                ..ServiceSpec::default()
            }),
            ..Service::default()
        }
    }

    fn deployment(&self, ctx: &Context, port: Option<i32>) -> Deployment {
        let name = self.name_any();
        let host_port = (ctx.config.game_servers.exposure.type_ == ExposureType::HostPort)
            .then(|| port.unwrap_or(GAME_PORT));
        // Content images are mounted in both containers, since the content links to them.
        let images: Vec<(usize, &String)> = (self.spec.content.iter().enumerate())
            .filter_map(|(i, content)| Some((i, content.image.as_ref()?)))
            .collect();
        let mut volume_mounts = vec![
            VolumeMount {
                name: "config".to_owned(),
                mount_path: "/config".to_owned(),
                read_only: Some(true),
                ..VolumeMount::default()
            },
            VolumeMount {
                name: "content".to_owned(),
                mount_path: "/assets".to_owned(),
                ..VolumeMount::default()
            },
        ];
        volume_mounts.extend(images.iter().map(|(i, _)| VolumeMount {
            name: format!("content-image-{i}"),
            mount_path: format!("{CONTENT_IMAGES_DIR}/{i}"),
            read_only: Some(true),
            ..VolumeMount::default()
        }));
        let mut volumes = vec![
            Volume {
                name: "config".to_owned(),
                secret: Some(SecretVolumeSource {
                    secret_name: Some(name.clone()),
                    ..SecretVolumeSource::default()
                }),
                ..Volume::default()
            },
            Volume {
                name: "content".to_owned(),
                empty_dir: Some(EmptyDirVolumeSource::default()),
                ..Volume::default()
            },
        ];
        volumes.extend(images.iter().map(|(i, image)| Volume {
            name: format!("content-image-{i}"),
            image: Some(ImageVolumeSource {
                reference: Some((*image).clone()),
                pull_policy: None,
            }),
            ..Volume::default()
        }));
        // Downloads the content before the server starts. The Secret it reads changes along
        // with the spec, so the checksum makes a change to what's downloaded replace the pod.
        let fetch = (!self.spec.content.is_empty()).then(|| Container {
            name: "content".to_owned(),
            image: Some(ctx.config.game_servers.image.clone()),
            command: Some(
                [
                    "q3",
                    "fetch",
                    "--config=/config/config.yaml",
                    "--assets-dir=/assets",
                ]
                .map(String::from)
                .to_vec(),
            ),
            volume_mounts: Some(volume_mounts.clone()),
            ..Container::default()
        });
        let fs_game = self
            .spec
            .game
            .as_ref()
            .and_then(|game| game.fs_game.as_ref());
        let annotations = (fs_game.is_some() || fetch.is_some()).then(|| {
            BTreeMap::from([(
                CONTENT_CHECKSUM_ANNOTATION.to_owned(),
                checksum(&(fs_game, &self.spec.content)),
            )])
        });
        Deployment {
            metadata: self.child_meta(name.clone()),
            spec: Some(DeploymentSpec {
                replicas: Some(1),
                // Stop the old server first so its host port and players are released before
                // the replacement starts.
                strategy: Some(DeploymentStrategy {
                    type_: Some("Recreate".to_owned()),
                    ..DeploymentStrategy::default()
                }),
                selector: LabelSelector {
                    match_labels: Some(self.selector_labels()),
                    ..LabelSelector::default()
                },
                template: PodTemplateSpec {
                    metadata: Some(ObjectMeta {
                        labels: Some(self.labels()),
                        annotations,
                        ..ObjectMeta::default()
                    }),
                    spec: Some(PodSpec {
                        init_containers: fetch.map(|fetch| vec![fetch]),
                        containers: vec![Container {
                            name: "server".to_owned(),
                            image: Some(ctx.config.game_servers.image.clone()),
                            command: Some(
                                [
                                    "q3",
                                    "run",
                                    "--config=/config/config.yaml",
                                    "--agree-eula",
                                    "--shutdown-delay=10s",
                                ]
                                .map(String::from)
                                .to_vec(),
                            ),
                            ports: Some(vec![
                                ContainerPort {
                                    name: Some("http".to_owned()),
                                    container_port: HTTP_PORT,
                                    ..ContainerPort::default()
                                },
                                ContainerPort {
                                    name: Some("game".to_owned()),
                                    container_port: GAME_PORT,
                                    host_port,
                                    protocol: Some("UDP".to_owned()),
                                    ..ContainerPort::default()
                                },
                            ]),
                            liveness_probe: Some(Probe {
                                initial_delay_seconds: Some(30),
                                period_seconds: Some(10),
                                failure_threshold: Some(3),
                                ..health_probe()
                            }),
                            readiness_probe: Some(Probe {
                                initial_delay_seconds: Some(5),
                                period_seconds: Some(3),
                                failure_threshold: Some(3),
                                ..health_probe()
                            }),
                            volume_mounts: Some(volume_mounts),
                            ..Container::default()
                        }],
                        volumes: Some(volumes),
                        ..PodSpec::default()
                    }),
                },
                ..DeploymentSpec::default()
            }),
            ..Deployment::default()
        }
    }

    /// Builds the status from the current state of the server's objects.
    async fn observe(
        &self,
        ctx: &Context,
        namespace: &str,
        port: Option<i32>,
    ) -> Result<QuakeServerStatus> {
        let client = &ctx.client;
        let name = self.name_any();
        let running = Api::<Deployment>::namespaced(client.clone(), namespace)
            .get_opt(&name)
            .await?
            .and_then(|deployment| deployment.status?.ready_replicas)
            .unwrap_or(0)
            > 0;

        let external = || async {
            Api::<Service>::namespaced(client.clone(), namespace)
                .get_opt(&format!("{name}-game"))
                .await
        };
        let address = match ctx.config.game_servers.exposure.type_ {
            ExposureType::LoadBalancer => external().await?.and_then(|svc| {
                let ingress = svc.status?.load_balancer?.ingress?.into_iter().next()?;
                let host = ingress.ip.or(ingress.hostname)?;
                Some(host_port(&host, svc.spec?.ports?.first()?.port))
            }),
            ExposureType::NodePort => {
                let node_port = external()
                    .await?
                    .and_then(|svc| svc.spec?.ports?.first()?.node_port);
                match (self.node_address(client, namespace).await?, node_port) {
                    (Some(host), Some(node_port)) => Some(host_port(&host, node_port)),
                    _ => None,
                }
            }
            ExposureType::HostPort => self
                .node_address(client, namespace)
                .await?
                .map(|host| host_port(&host, port.unwrap_or(GAME_PORT))),
            ExposureType::None => None,
        };

        Ok(QuakeServerStatus {
            ready: running,
            address,
            port,
            observed_generation: self.metadata.generation,
        })
    }

    /// Returns the address of the node running the server, preferring its external IP.
    async fn node_address(&self, client: &Client, namespace: &str) -> Result<Option<String>> {
        let selector = self
            .selector_labels()
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(",");
        let pods = Api::<Pod>::namespaced(client.clone(), namespace)
            .list(&ListParams::default().labels(&selector))
            .await?;
        let Some(node_name) = pods.into_iter().find_map(|pod| pod.spec?.node_name) else {
            return Ok(None);
        };
        let addresses = Api::<Node>::all(client.clone())
            .get(&node_name)
            .await?
            .status
            .and_then(|status| status.addresses)
            .unwrap_or_default();
        Ok(["ExternalIP", "InternalIP"].iter().find_map(|type_| {
            addresses
                .iter()
                .find(|addr| addr.type_ == *type_)
                .map(|addr| addr.address.clone())
        }))
    }

    fn selector_labels(&self) -> BTreeMap<String, String> {
        BTreeMap::from([
            (
                "app.kubernetes.io/name".to_owned(),
                "quake-server".to_owned(),
            ),
            ("app.kubernetes.io/instance".to_owned(), self.name_any()),
        ])
    }

    fn labels(&self) -> BTreeMap<String, String> {
        let mut labels = self.selector_labels();
        labels.insert(
            "app.kubernetes.io/managed-by".to_owned(),
            FIELD_MANAGER.to_owned(),
        );
        labels
    }

    fn child_meta(&self, name: String) -> ObjectMeta {
        ObjectMeta {
            name: Some(name),
            namespace: self.namespace(),
            labels: Some(self.labels()),
            owner_references: self.controller_owner_ref(&()).map(|oref| vec![oref]),
            ..ObjectMeta::default()
        }
    }
}

/// Joins a host and port, bracketing IPv6 addresses.
fn host_port(host: &str, port: i32) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// A stable checksum of value (FNV-1a over its JSON), unlike std's hashers, whose output can
/// change between Rust releases and would replace every pod after an upgrade.
fn checksum(value: &impl Serialize) -> String {
    let json = serde_json::to_vec(value).unwrap_or_default();
    let hash = json.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}")
}

fn health_probe() -> Probe {
    Probe {
        grpc: Some(GRPCAction {
            port: HTTP_PORT,
            service: None,
        }),
        success_threshold: Some(1),
        ..Probe::default()
    }
}

/// Creates or updates obj with server-side apply.
async fn apply<K>(client: &Client, namespace: &str, obj: &K) -> Result<K>
where
    K: Resource<Scope = NamespaceResourceScope>
        + Clone
        + std::fmt::Debug
        + Serialize
        + DeserializeOwned,
    K::DynamicType: Default,
{
    Ok(Api::<K>::namespaced(client.clone(), namespace)
        .patch(
            &obj.name_any(),
            &PatchParams::apply(FIELD_MANAGER).force(),
            &Patch::Apply(obj),
        )
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(spec: serde_json::Value) -> QuakeServer {
        let mut server = QuakeServer::new("test", serde_json::from_value(spec).unwrap());
        server.metadata.namespace = Some("default".to_owned());
        server
    }

    #[test]
    fn renders_mod_config() {
        let server = server(json!({
            "game": { "type": 7, "fsGame": "q3ut4" },
            "maps": [{ "name": "ut4_casa" }, { "name": "ut4_turnpike", "type": "FreeForAll" }],
            "cvars": { "g_gear": "0" },
            "content": [
                { "url": "https://example.com/urt.zip", "stripComponents": 1 },
                { "image": "example.com/maps:1" },
            ],
        }));
        let config: serde_json::Value =
            serde_yaml_ng::from_str(&server.render_config(None, "secret").unwrap()).unwrap();
        assert_eq!(config["fs"]["game"], "q3ut4");
        assert!(config["game"].get("fsGame").is_none());
        assert_eq!(config["maps"][0]["type"], 7);
        assert_eq!(config["maps"][1]["type"], "FreeForAll");
        assert_eq!(config["cvars"]["g_gear"], "0");
        assert_eq!(config["content"][0]["stripComponents"], 1);
        assert!(config["content"][0].get("path").is_none());
        assert_eq!(config["content"][1]["path"], "/content-images/1");
        assert_eq!(config["server"]["password"], "secret");
    }

    #[test]
    fn content_checksum_is_stable() {
        // Pinned, since a change would replace every server's pod.
        assert_eq!(checksum(&"a"), "d4272417d7c77eea");
        let content = |url: &str| server(json!({ "content": [{ "url": url }] })).spec.content;
        assert_ne!(
            checksum(&(Some("cpma"), &content("https://example.com/a.zip"))),
            checksum(&(Some("cpma"), &content("https://example.com/b.zip"))),
        );
    }
}
