//! The gateway serves the web app, which lists servers and lets people create them and play
//! in the browser. Browser clients can't send UDP, so their game traffic arrives over a
//! WebSocket and is relayed to the server here.

use crate::auth::{Auth, AuthError, User};
use crate::config::{ExposureType, GatewayConfig};
use crate::config::{MapList, Preset, PresetGameType};
use crate::levelshots::{Arena, Levelshots};
use crate::prelude::*;
use crate::protocol;
use crate::quake_server::{MapSpec, QuakeServerSpec};
use crate::quake_server::{GAME_PORT, HTTP_PORT};
use crate::registry::{self, Registry};
use crate::webtransport::WebTransport;
use axum::body::{Body, Bytes};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{FromRequestParts, Path, RawQuery, State};
use axum::http::{header, request::Parts, Method, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use futures::SinkExt;
use k8s_openapi::jiff::Timestamp;
use kube::api::PostParams;
use kube::runtime::reflector::{self, ObjectRef, Store};
use kube::runtime::WatchStreamExt;
use serde_json::json;
use tokio::net::{TcpListener, UdpSocket};
use tracing::*;

/// Label on servers created from the web app. Only these can be changed or deleted from it.
const CREATED_BY_LABEL: &str = "quake.kube/created-by";

/// Label with the preset a server was created from.
const PRESET_LABEL: &str = "quake.kube/preset";

/// Annotation with the email address of whoever created a server from the web app, when the
/// gateway knows who people are.
const OWNER_ANNOTATION: &str = "quake.kube/owner";

/// Annotation with the name of the preset's game type a server was created with.
const GAME_TYPE_ANNOTATION: &str = "quake.kube/game-type";

/// Annotation with when a server created from the web app was last left without players, so
/// every gateway replica counts down to deleting it from the same time.
const IDLE_SINCE_ANNOTATION: &str = "quake.kube/idle-since";

/// How often servers created from the web app are checked for players.
const IDLE_CHECK_INTERVAL: Duration = Duration::from_secs(30);

const INDEX_HTML: &str = include_str!("../static/index.html");
const PLAY_HTML: &str = include_str!("../static/play.html");
const GAMESOCKET_JS: &str = include_str!("../static/gamesocket.js");
const PAKS_JS: &str = include_str!("../static/paks.js");
const MAPS_JS: &str = include_str!("../static/maps.js");
const EULA_TXT: &str = include_str!("../static/eula.txt");

/// The ioquake3 web client built into the image.
struct WebClient {
    /// Changes whenever the files do. Pages ask for the files by version, so they can be
    /// cached for good.
    version: String,
    js: Bytes,
    wasm: Bytes,
    /// A build that reads paks on demand, for browsers with JSPI.
    jspi: Option<(Bytes, Bytes)>,
}

impl WebClient {
    fn load(dir: &std::path::Path) -> std::io::Result<Self> {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let js = std::fs::read(dir.join("ioquake3.js"))?;
        let wasm = std::fs::read(dir.join("ioquake3.wasm"))?;
        let jspi = std::fs::read(dir.join("jspi/ioquake3.js"))
            .and_then(|js| Ok((js, std::fs::read(dir.join("jspi/ioquake3.wasm"))?)))
            .ok();
        let mut hasher = DefaultHasher::new();
        js.hash(&mut hasher);
        wasm.hash(&mut hasher);
        jspi.hash(&mut hasher);
        Ok(WebClient {
            version: format!("{:016x}", hasher.finish()),
            js: js.into(),
            wasm: wasm.into(),
            jspi: jspi.map(|(js, wasm)| (js.into(), wasm.into())),
        })
    }
}

struct Gateway {
    client: Client,
    config: GatewayConfig,
    store: Store<QuakeServer>,
    registry: Arc<Registry>,
    http: reqwest::Client,
    webtransport: Option<Arc<WebTransport>>,
    web_client: Option<WebClient>,
    auth: Arc<Auth>,
    /// Whether game servers have external addresses native clients can connect to.
    native_clients: bool,
    /// The master server's address, which game servers' configs name, so the gateway renders
    /// the same configs as the controller.
    master: Option<String>,
    /// Thumbnails of every preset's maps.
    levelshots: Arc<Levelshots>,
}

impl Gateway {
    /// Returns the QuakeServer the request is for. Requests are only proxied to known servers,
    /// so the gateway can't be used to reach anything else in the cluster.
    fn server(&self, namespace: &str, name: &str) -> Result<Arc<QuakeServer>, StatusCode> {
        self.store
            .get(&ObjectRef::new(name).within(namespace))
            .ok_or(StatusCode::NOT_FOUND)
    }
}

/// Who made a request. None when the gateway doesn't know who people are, so anyone can do
/// anything. Requests from people who aren't signed in are turned away.
struct Identity(Option<User>);

impl FromRequestParts<Arc<Gateway>> for Identity {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        gateway: &Arc<Gateway>,
    ) -> Result<Self, Self::Rejection> {
        match gateway.auth.user(&parts.headers).await {
            Ok(user) => Ok(Identity(user)),
            Err(AuthError::Missing) => Err((StatusCode::UNAUTHORIZED, "not signed in")),
            Err(error) => {
                debug!("rejected request: {error}");
                Err((StatusCode::UNAUTHORIZED, "not signed in"))
            }
        }
    }
}

/// Whether the user can change or delete a server from the web app: one created there, by
/// them unless they're an admin.
fn manageable(server: &QuakeServer, user: Option<&User>, admin: bool) -> bool {
    if !created_here(server) {
        return false;
    }
    match user {
        None => true,
        Some(user) => admin || owner(server) == Some(user.email.as_str()),
    }
}

fn created_here(server: &QuakeServer) -> bool {
    server.labels().get(CREATED_BY_LABEL).map(String::as_str) == Some("gateway")
}

/// Checks the user can change or delete a server, only asking the API server whether they're
/// an admin when it matters.
async fn authorize(
    gateway: &Gateway,
    server: &QuakeServer,
    user: Option<&User>,
    namespace: &str,
) -> Result<(), (StatusCode, String)> {
    if manageable(server, user, false) {
        return Ok(());
    }
    let admin = match user {
        Some(user) => gateway.auth.is_admin(user, namespace).await,
        None => false,
    };
    if manageable(server, user, admin) {
        return Ok(());
    }
    Err((
        StatusCode::FORBIDDEN,
        "only servers created here can be changed, by whoever created them or an admin".to_owned(),
    ))
}

/// How many people, not bots, are playing on a server, as of its latest info.
fn humans(gateway: &Gateway, server: &QuakeServer) -> i32 {
    gateway
        .registry
        .get(&ObjectRef::from_obj(server))
        .and_then(|entry| entry.info.get("g_humanplayers")?.parse().ok())
        .unwrap_or(0)
}

/// The game a server runs, as shown in the web app: the preset it was created from, or its mod.
fn game_title(gateway: &Gateway, server: &QuakeServer) -> Option<String> {
    let preset = (server.labels().get(PRESET_LABEL))
        .and_then(|name| gateway.config.presets.iter().find(|p| &p.name == name));
    preset.map(|preset| preset.title.clone()).or_else(|| {
        server
            .spec
            .game
            .as_ref()
            .and_then(|game| game.fs_game.clone())
    })
}

fn idle_since(server: &QuakeServer) -> Option<Timestamp> {
    server
        .annotations()
        .get(IDLE_SINCE_ANNOTATION)?
        .parse()
        .ok()
}

fn owner(server: &QuakeServer) -> Option<&str> {
    server
        .annotations()
        .get(OWNER_ANNOTATION)
        .map(String::as_str)
}

pub async fn run(client: Client, config: Config) -> anyhow::Result<()> {
    // The gateway's HTTPS requests, e.g. for IAP's keys, use the same crypto as WebTransport.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let auth = Arc::new(Auth::new(config.gateway.auth.clone(), client.clone())?);
    let (store, writer) = reflector::store();
    let registry = Registry::new(store.clone());
    tokio::spawn(registry.clone().poll());

    // Refresh the server info whenever a server changes, e.g. when one becomes ready.
    let watch = reflector::reflector(
        writer,
        watcher(Api::<QuakeServer>::all(client.clone()), Default::default()),
    )
    .default_backoff()
    .touched_objects()
    .for_each({
        let registry = registry.clone();
        move |res| {
            match res {
                Ok(_) => registry.refresh(),
                Err(error) => warn!("watching QuakeServers failed: {error}"),
            }
            futures::future::ready(())
        }
    });
    tokio::spawn(watch);

    let webtransport = if config.gateway.webtransport.enabled {
        let webtransport = WebTransport::new(config.gateway.webtransport.clone()).await?;
        tokio::spawn(webtransport.clone().serve(store.clone(), auth.clone()));
        tokio::spawn(webtransport.clone().rotate_certificates());
        Some(webtransport)
    } else {
        None
    };

    let web_client = match WebClient::load(&config.gateway.client_dir) {
        Ok(client) => Some(client),
        Err(error) => {
            warn!(dir = %config.gateway.client_dir.display(), "web client unavailable: {error}");
            None
        }
    };

    let port = config.gateway.port;
    let levelshots = Levelshots::new(
        client.clone(),
        config.gateway.namespace.clone(),
        config.game_servers.image.clone(),
    );
    // Only needed for creating and changing servers, which the gateway can't when it's off.
    if config.gateway.max_servers > 0 {
        tokio::spawn(levelshots.clone().run(config.gateway.presets.clone()));
    }
    let native_clients = config.game_servers.exposure.type_ != ExposureType::None;
    let gateway = Arc::new(Gateway {
        client,
        config: config.gateway,
        store,
        registry,
        http: reqwest::Client::new(),
        webtransport,
        web_client,
        auth,
        native_clients,
        master: config.master.address,
        levelshots,
    });
    tokio::spawn(delete_idle_servers(gateway.clone()));

    let app = Router::new()
        .route("/", get(|| async { Html(INDEX_HTML) }))
        .route("/healthz", get(|| async { "ok" }))
        .route(
            "/static/gamesocket.js",
            get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], GAMESOCKET_JS) }),
        )
        .route(
            "/static/paks.js",
            get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], PAKS_JS) }),
        )
        .route(
            "/static/maps.js",
            get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], MAPS_JS) }),
        )
        .route(
            "/static/eula.txt",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    EULA_TXT,
                )
            }),
        )
        .route("/client/{file}", get(client_file))
        .route("/client/jspi/{file}", get(jspi_client_file))
        .route("/api/config", get(app_config))
        .route("/api/servers", get(list_servers).post(create_server))
        .route(
            "/api/servers/{namespace}/{name}",
            axum::routing::patch(update_server).delete(delete_server),
        )
        .route("/api/servers/{namespace}/{name}/status", get(server_status))
        .route(
            "/api/presets/{preset}/levelshots/{file}",
            get(preset_levelshot),
        )
        .route("/play/{namespace}/{name}", get(play))
        .route("/play/{namespace}/{name}/", get(play))
        .route("/play/{namespace}/{name}/ws", get(relay))
        .route("/play/{namespace}/{name}/{*path}", get(proxy).post(proxy))
        .with_state(gateway);

    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    info!("gateway listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Deletes the servers created from the web app, when the chart is uninstalled. Kubernetes
/// deletes the Deployments and Services they own. Servers created otherwise, e.g. with
/// kubectl, are left alone.
pub async fn cleanup(client: Client, config: Config) -> anyhow::Result<()> {
    let api = Api::<QuakeServer>::namespaced(client, &config.gateway.namespace);
    let created = ListParams::default().labels(&format!("{CREATED_BY_LABEL}=gateway"));
    for server in api.list(&created).await? {
        let name = server.name_any();
        info!(server = %name, "deleting server created from the web app");
        api.delete(&name, &DeleteParams::background()).await?;
    }
    Ok(())
}

/// Deletes servers created from the web app once nobody has played on them for the idle
/// timeout. A server that never starts, e.g. because its content fails to download, has no
/// players either, so it's cleaned up too.
async fn delete_idle_servers(gateway: Arc<Gateway>) {
    let Some(idle_timeout) = gateway.config.idle_timeout else {
        return;
    };
    let api = Api::<QuakeServer>::namespaced(gateway.client.clone(), &gateway.config.namespace);
    loop {
        // Waiting first gives the registry time to hear from the servers after a restart.
        tokio::time::sleep(IDLE_CHECK_INTERVAL).await;
        let now = Timestamp::now();
        for server in gateway.store.state() {
            if !created_here(&server)
                || server.namespace().as_deref() != Some(gateway.config.namespace.as_str())
            {
                continue;
            }
            let name = server.name_any();
            let since = idle_since(&server);
            // The annotation's new value, null to remove it.
            let annotation = if humans(&gateway, &server) > 0 {
                since.is_some().then_some(serde_json::Value::Null)
            } else {
                match since {
                    None => Some(now.to_string().into()),
                    Some(since)
                        if now.as_second() - since.as_second() >= idle_timeout.as_secs() as i64 =>
                    {
                        info!(server = %name, owner = owner(&server), "deleting idle server");
                        if let Err(error) = api.delete(&name, &DeleteParams::default()).await {
                            warn!(server = %name, "deleting idle server failed: {error}");
                        }
                        None
                    }
                    Some(_) => None,
                }
            };
            if let Some(value) = annotation {
                let patch =
                    json!({ "metadata": { "annotations": { IDLE_SINCE_ANNOTATION: value } } });
                if let Err(error) = api
                    .patch(&name, &PatchParams::default(), &Patch::Merge(patch))
                    .await
                {
                    warn!(server = %name, "updating idle time failed: {error}");
                }
            }
        }
    }
}

/// Resolves on Ctrl-C, or on SIGTERM, which Kubernetes sends to stop the pod.
async fn shutdown_signal() {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("installing SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
}

/// What the web app offers when creating servers.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppConfig<'a> {
    /// Who's signed in, when the gateway knows.
    user: Option<String>,
    can_create: bool,
    /// Whether native clients can connect to servers' addresses.
    native_clients: bool,
    /// How many servers can be created from the web app at once, and by each person.
    max_servers: usize,
    max_servers_per_user: Option<usize>,
    /// Seconds without players before a server created from the web app is deleted.
    idle_timeout: Option<u64>,
    presets: Vec<PresetView<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PresetView<'a> {
    name: &'a str,
    title: &'a str,
    game_types: Vec<GameTypeView<'a>>,
    maps: &'a [String],
    bots: bool,
    /// What the maps' arena files say about them, by lowercase map name, once known.
    map_info: BTreeMap<String, Arena>,
    map_lists: &'a [MapList],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GameTypeView<'a> {
    name: &'a str,
    /// The game type maps' arena files name for it, e.g. "ctf", when known.
    arena: Option<&'a str>,
}

async fn app_config(State(gateway): State<Arc<Gateway>>, Identity(user): Identity) -> Response {
    let config = &gateway.config;
    let user_known = user.is_some();
    Json(AppConfig {
        user: user.map(|user| user.email),
        native_clients: gateway.native_clients,
        can_create: config.max_servers > 0 && !config.presets.is_empty(),
        max_servers: config.max_servers,
        // Only applies when the gateway knows who people are.
        max_servers_per_user: user_known.then_some(config.max_servers_per_user).flatten(),
        idle_timeout: config.idle_timeout.map(|timeout| timeout.as_secs()),
        presets: preset_views(&gateway),
    })
    .into_response()
}

fn preset_views(gateway: &Gateway) -> Vec<PresetView<'_>> {
    gateway
        .config
        .presets
        .iter()
        .map(|preset| PresetView {
            name: &preset.name,
            title: &preset.title,
            game_types: (preset.game_types.iter())
                .map(|t| GameTypeView {
                    name: &t.name,
                    arena: t.arena_type(),
                })
                .collect(),
            maps: &preset.maps,
            bots: preset.bots,
            map_info: gateway.levelshots.arenas(&preset.name),
            map_lists: &preset.map_lists,
        })
        .collect()
}

/// A server as shown in the web app.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerView {
    namespace: String,
    name: String,
    /// The preset it was created from, or the mod it runs.
    game: Option<String>,
    ready: bool,
    /// Address for native clients.
    address: Option<String>,
    /// Filled in once the server answers queries.
    hostname: Option<String>,
    map: Option<String>,
    /// How many maps it plays in turn.
    rotation: usize,
    game_type: Option<String>,
    humans: Option<i32>,
    bots: Option<i32>,
    max_players: Option<i32>,
    needs_password: bool,
    /// Why it isn't running, when something's wrong rather than it still starting.
    message: Option<String>,
    /// Who created it from the web app, when the gateway knows who people are.
    owner: Option<String>,
    /// Whether it was created from the web app, which counts towards its limits.
    created_here: bool,
    /// Whether the signed-in user created it.
    mine: bool,
    /// Whether the user can change or delete it from the web app.
    manageable: bool,
    /// Its settings, for changing them, when the user can.
    settings: Option<ServerSettings>,
    /// When it'll be deleted for having no players, in milliseconds since the epoch.
    deletes_at: Option<i64>,
}

async fn list_servers(
    State(gateway): State<Arc<Gateway>>,
    Identity(user): Identity,
) -> Json<Vec<ServerView>> {
    // Servers are only created from the web app in its own namespace.
    let admin = match &user {
        Some(user) => gateway.auth.is_admin(user, &gateway.config.namespace).await,
        None => false,
    };
    let now = Timestamp::now();
    let mut servers: Vec<ServerView> = gateway
        .store
        .state()
        .iter()
        .map(|qs| {
            let in_namespace = qs.namespace().as_deref() == Some(gateway.config.namespace.as_str());
            let manageable = manageable(qs, user.as_ref(), admin && in_namespace);
            let deletes_at = gateway
                .config
                .idle_timeout
                .filter(|_| created_here(qs) && in_namespace && humans(&gateway, qs) == 0);
            let deletes_at = deletes_at.map(|timeout| {
                idle_since(qs).unwrap_or(now).as_millisecond() + timeout.as_millis() as i64
            });
            let entry = gateway.registry.get(&ObjectRef::from_obj(qs.as_ref()));
            let info = entry.as_ref().map(|e| &e.info);
            let get = |key: &str| info.and_then(|info| info.get(key)).cloned();
            let number = |key: &str| get(key).and_then(|v| v.parse::<i32>().ok());
            let humans = number("g_humanplayers");
            let status = qs.status.clone().unwrap_or_default();
            ServerView {
                namespace: qs.namespace().unwrap_or_default(),
                name: qs.name_any(),
                game: game_title(&gateway, qs),
                ready: status.ready,
                address: status.address,
                // Until the server answers, its spec has the name it was created with.
                hostname: get("hostname").or_else(|| {
                    qs.spec
                        .server
                        .as_ref()
                        .and_then(|server| server.hostname.clone())
                }),
                map: get("mapname"),
                rotation: qs.spec.maps.len(),
                // Mods that choose their modes by cvars, like DeFRaG, are named by the
                // preset's game type the server was created with.
                game_type: number("gametype")
                    .and_then(|n| game_type_name(get("game").as_deref(), n))
                    .map(String::from)
                    .or_else(|| qs.annotations().get(GAME_TYPE_ANNOTATION).cloned()),
                humans,
                bots: number("clients")
                    .zip(humans)
                    .map(|(all, humans)| all - humans),
                max_players: number("sv_maxclients"),
                needs_password: needs_password(qs, get("g_needpass").as_deref()),
                message: status.message.filter(|_| !status.ready),
                owner: owner(qs).map(String::from),
                created_here: created_here(qs),
                mine: user
                    .as_ref()
                    .is_some_and(|user| owner(qs) == Some(user.email.as_str())),
                manageable,
                settings: manageable.then(|| ServerSettings::of(qs)),
                deletes_at,
            }
        })
        .collect();
    servers.sort_by(|a, b| (&a.namespace, &a.name).cmp(&(&b.namespace, &b.name)));
    Json(servers)
}

/// Whether players need a password to join. The game module in the Quake 3 demo doesn't report
/// g_needpass, so the spec is checked too.
fn needs_password(server: &QuakeServer, needpass: Option<&str>) -> bool {
    needpass == Some("1")
        || server
            .spec
            .game
            .as_ref()
            .is_some_and(|game| game.password.is_some())
}

/// Names a server's game type. Mods number their game types differently, so the names
/// depend on the server's game.
fn game_type_name(game: Option<&str>, game_type: i32) -> Option<&'static str> {
    Some(match (game, game_type) {
        (Some("q3ut4"), 0) => "Free For All",
        (Some("q3ut4"), 1) => "Last Man Standing",
        (Some("q3ut4"), 3) => "Team Deathmatch",
        (Some("q3ut4"), 4) => "Team Survivor",
        (Some("q3ut4"), 5) => "Follow The Leader",
        (Some("q3ut4"), 6) => "Capture And Hold",
        (Some("q3ut4"), 7) => "Capture The Flag",
        (Some("q3ut4"), 8) => "Bomb Mode",
        (Some("q3ut4"), 9) => "Jump",
        (Some("q3ut4"), 10) => "Freeze Tag",
        (Some("q3ut4"), 11) => "Gun Game",
        (Some("q3ut4"), _) => return None,
        // DeFRaG leaves g_gametype alone and chooses its mode by df_gametype.
        (Some("defrag"), _) => return None,
        (Some("ufreeze"), 3) => "Team Freeze",
        (Some("ufreeze"), 4) => "CTF Freeze",
        (Some("cpma"), -1) => "Hoonymode",
        (Some("cpma"), 1) => "Duel",
        (Some("cpma"), 2) => "Duel Arena",
        (Some("cpma"), 5) => "Clan Arena",
        (Some("cpma"), 6) => "Freeze Tag",
        (Some("cpma"), 7) => "Capture Strike",
        (Some("cpma"), 8) => "2v2",
        (_, 0) => "Free For All",
        (_, 1) => "Tournament",
        (_, 2) => "Single Player",
        (_, 3) => "Team Deathmatch",
        (_, 4) => "Capture The Flag",
        _ => return None,
    })
}

/// A server's settings in the web app, which it creates and changes servers with.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ServerSettings {
    name: String,
    /// The game, which can't be changed once the server's created.
    #[serde(default)]
    preset: String,
    game_type: String,
    /// The maps the server plays in turn, a new one after each match.
    #[serde(default)]
    maps: Vec<String>,
    #[serde(default)]
    bots: i32,
    #[serde(default)]
    password: Option<String>,
    /// Limits for each map, or the game's defaults when unset.
    #[serde(default)]
    frag_limit: Option<i32>,
    /// In minutes.
    #[serde(default)]
    time_limit: Option<i32>,
    #[serde(default)]
    capture_limit: Option<i32>,
    #[serde(default)]
    max_players: Option<i32>,
}

impl ServerSettings {
    /// The settings a server was created with, or last changed to.
    fn of(server: &QuakeServer) -> Self {
        let spec = &server.spec;
        let map = spec.maps.first();
        ServerSettings {
            name: (spec.server.as_ref())
                .and_then(|server| server.hostname.clone())
                .unwrap_or_else(|| server.name_any()),
            preset: (server.labels().get(PRESET_LABEL).cloned()).unwrap_or_default(),
            game_type: (server.annotations().get(GAME_TYPE_ANNOTATION).cloned())
                .unwrap_or_default(),
            maps: spec.maps.iter().map(|map| map.name.clone()).collect(),
            bots: (spec.bot.as_ref())
                .and_then(|bot| bot.min_players)
                .unwrap_or(0),
            password: spec.game.as_ref().and_then(|game| game.password.clone()),
            frag_limit: map.and_then(|map| map.frag_limit),
            time_limit: (map.and_then(|map| map.time_limit.as_deref()))
                .and_then(|limit| limit.strip_suffix('m')?.parse().ok()),
            capture_limit: map.and_then(|map| map.capture_limit),
            max_players: spec.server.as_ref().and_then(|server| server.max_clients),
        }
    }

    /// Checks the settings against the preset, returning the game type and the server's display
    /// name.
    fn validate<'a>(
        &self,
        preset: &'a Preset,
    ) -> Result<(&'a PresetGameType, String), (StatusCode, String)> {
        let bad_request = |msg: String| Err((StatusCode::BAD_REQUEST, msg));
        let hostname = display_name(&self.name);
        if hostname.is_empty() || hostname.chars().count() > MAX_DISPLAY_NAME {
            return bad_request(format!("names are 1 to {MAX_DISPLAY_NAME} characters"));
        }
        let Some(game_type) = preset.game_types.iter().find(|t| t.name == self.game_type) else {
            return bad_request("unknown game type".to_owned());
        };
        if self.maps.is_empty() || self.maps.len() > MAX_ROTATION {
            return bad_request(format!("choose 1 to {MAX_ROTATION} maps"));
        }
        if let Some(map) = self.maps.iter().find(|map| !preset.maps.contains(map)) {
            return bad_request(format!("unknown map {map}"));
        }
        let limits = [
            ("bots", Some(self.bots), 0, 16),
            ("the frag limit", self.frag_limit, 0, 999),
            ("the time limit", self.time_limit, 0, 999),
            ("the capture limit", self.capture_limit, 0, 999),
            ("max players", self.max_players, 0, 64),
        ];
        for (what, value, min, max) in limits {
            if value.is_some_and(|value| !(min..=max).contains(&value)) {
                return bad_request(format!("{what} must be between {min} and {max}"));
            }
        }
        Ok((game_type, hostname))
    }

    /// The server's spec: the preset's, with these settings.
    fn spec(&self, preset: &Preset, game_type: &PresetGameType, hostname: &str) -> QuakeServerSpec {
        let mut spec = preset.spec.clone();
        if preset.bots {
            spec.bot.get_or_insert_default().min_players = Some(self.bots);
        }
        let game = spec.game.get_or_insert_default();
        if game_type.type_.is_some() {
            game.type_ = game_type.type_;
        }
        game.password = self.password.clone().filter(|p| !p.is_empty());
        spec.cvars.extend(game_type.cvars.clone());
        let server = spec.server.get_or_insert_default();
        server.hostname = Some(hostname.to_owned());
        // 0 means the game's default, like an unset limit.
        let set = |value: Option<i32>| value.filter(|&value| value > 0);
        if let Some(max_players) = set(self.max_players) {
            server.max_clients = Some(max_players);
        }
        spec.maps = (self.maps.iter())
            .map(|name| MapSpec {
                name: name.clone(),
                type_: None,
                capture_limit: set(self.capture_limit),
                frag_limit: set(self.frag_limit),
                time_limit: set(self.time_limit).map(|minutes| format!("{minutes}m")),
            })
            .collect();
        spec
    }
}

async fn create_server(
    State(gateway): State<Arc<Gateway>>,
    Identity(user): Identity,
    Json(request): Json<ServerSettings>,
) -> Result<StatusCode, (StatusCode, String)> {
    let config = &gateway.config;
    if config.max_servers == 0 {
        return Err((
            StatusCode::FORBIDDEN,
            "creating servers is turned off".to_owned(),
        ));
    }
    let Some(preset) = config.presets.iter().find(|p| p.name == request.preset) else {
        return Err((StatusCode::BAD_REQUEST, "unknown game".to_owned()));
    };
    let (game_type, hostname) = request.validate(preset)?;
    let created: Vec<_> = gateway
        .store
        .state()
        .into_iter()
        .filter(|qs| qs.labels().contains_key(CREATED_BY_LABEL))
        .collect();
    if let (Some(limit), Some(user)) = (config.max_servers_per_user, &user) {
        let owned = created
            .iter()
            .filter(|qs| owner(qs) == Some(user.email.as_str()))
            .count();
        if owned >= limit {
            return Err((
                StatusCode::CONFLICT,
                format!(
                    "each person can have up to {limit} {} at once",
                    if limit == 1 { "server" } else { "servers" }
                ),
            ));
        }
    }
    if created.len() >= config.max_servers {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "the limit of {} servers has been reached",
                config.max_servers
            ),
        ));
    }

    let spec = request.spec(preset, game_type, &hostname);
    // Servers can share a display name, so the first free name, with a number if needed, is
    // used for the resources.
    let api = Api::<QuakeServer>::namespaced(gateway.client.clone(), &config.namespace);
    let base = resource_name(&hostname);
    for n in 1..=20 {
        let name = numbered_name(&base, n);
        let mut server = QuakeServer::new(&name, spec.clone());
        let labels = server.labels_mut();
        labels.insert(CREATED_BY_LABEL.to_owned(), "gateway".to_owned());
        labels.insert(PRESET_LABEL.to_owned(), preset.name.clone());
        let annotations = server.annotations_mut();
        annotations.insert(GAME_TYPE_ANNOTATION.to_owned(), game_type.name.clone());
        if let Some(user) = &user {
            annotations.insert(OWNER_ANNOTATION.to_owned(), user.email.clone());
        }
        match api.create(&PostParams::default(), &server).await {
            Ok(_) => {
                info!(
                    server = %name,
                    %hostname,
                    user = user.as_ref().map(|user| user.email.as_str()),
                    "created server"
                );
                return Ok(StatusCode::CREATED);
            }
            Err(kube::Error::Api(status)) if status.code == 409 => continue,
            Err(error) => {
                warn!("creating server failed: {error}");
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "creating the server failed".to_owned(),
                ));
            }
        }
    }
    Err((
        StatusCode::CONFLICT,
        format!("too many servers are called {hostname}"),
    ))
}

/// The most maps a server's rotation can have.
const MAX_ROTATION: usize = 64;

/// The longest display name a server can have.
const MAX_DISPLAY_NAME: usize = 64;

/// The longest name for a server's resources, leaving room for the "-game" suffix of its
/// Service's name.
const MAX_RESOURCE_NAME: usize = 40;

/// A server's display name, which players see as its hostname, without what the game's
/// config and info strings can't hold.
fn display_name(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control() && !matches!(c, '"' | '\\'))
        .collect::<String>()
        .trim()
        .to_owned()
}

/// A name for a server's resources from its display name: its letters and digits, lowercase,
/// with dashes between words, e.g. "friday-night" for "Friday Night!".
fn resource_name(display: &str) -> String {
    let mut name = String::new();
    for c in display.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            name.push(c);
        } else if !name.is_empty() && !name.ends_with('-') {
            name.push('-');
        }
    }
    let name = name.trim_end_matches('-');
    if name.is_empty() {
        "server".to_owned()
    } else {
        name.to_owned()
    }
}

/// The nth name to try for a server, e.g. "friday-night", then "friday-night-2".
fn numbered_name(base: &str, n: u32) -> String {
    let suffix = if n == 1 {
        String::new()
    } else {
        format!("-{n}")
    };
    let base: String = base
        .chars()
        .take(MAX_RESOURCE_NAME - suffix.len())
        .collect();
    format!("{}{suffix}", base.trim_end_matches('-'))
}

async fn delete_server(
    State(gateway): State<Arc<Gateway>>,
    Identity(user): Identity,
    Path((namespace, name)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let server = gateway
        .server(&namespace, &name)
        .map_err(|status| (status, "no such server".to_owned()))?;
    authorize(&gateway, &server, user.as_ref(), &namespace).await?;
    Api::<QuakeServer>::namespaced(gateway.client.clone(), &namespace)
        .delete(&name, &DeleteParams::default())
        .await
        .map_err(|error| {
            warn!("deleting server failed: {error}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "deleting the server failed".to_owned(),
            )
        })?;
    info!(
        %namespace,
        server = %name,
        user = user.as_ref().map(|user| user.email.as_str()),
        owner = owner(&server),
        "deleted server"
    );
    Ok(StatusCode::NO_CONTENT)
}

/// How a change to a server's settings took effect.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
enum Applied {
    /// Right away, from the server's console, which reloads the map.
    Live,
    /// When the server reloads its config, within a minute or two, since sending it right away
    /// failed.
    Later,
    /// By replacing the server, for a different game or its content.
    Restart,
}

#[derive(Serialize)]
struct UpdateResult {
    applied: Applied,
}

/// Changes a server's settings, including which game it runs. Changes the game can make while
/// running are sent to the server right away, besides being saved in its spec.
async fn update_server(
    State(gateway): State<Arc<Gateway>>,
    Identity(user): Identity,
    Path((namespace, name)): Path<(String, String)>,
    Json(request): Json<ServerSettings>,
) -> Result<Json<UpdateResult>, (StatusCode, String)> {
    let server = gateway
        .server(&namespace, &name)
        .map_err(|status| (status, "no such server".to_owned()))?;
    authorize(&gateway, &server, user.as_ref(), &namespace).await?;
    // The game stays the same unless the request names another.
    let preset_name = match request.preset.as_str() {
        "" => server
            .labels()
            .get(PRESET_LABEL)
            .cloned()
            .unwrap_or_default(),
        name => name.to_owned(),
    };
    let preset = (gateway.config.presets.iter())
        .find(|p| p.name == preset_name)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "this game isn't offered anymore".to_owned(),
            )
        })?;
    let (game_type, hostname) = request.validate(preset)?;

    // Replaced rather than patched, so the old game type's cvars go, and with the version
    // from the cache, so a change made meanwhile isn't overwritten.
    let mut updated = (*server).clone();
    updated.spec = request.spec(preset, game_type, &hostname);
    updated.metadata.managed_fields = None;
    updated
        .annotations_mut()
        .insert(GAME_TYPE_ANNOTATION.to_owned(), game_type.name.clone());
    updated
        .labels_mut()
        .insert(PRESET_LABEL.to_owned(), preset.name.clone());
    // With its new generation, which the config sent to the server is stamped with.
    let updated = Api::<QuakeServer>::namespaced(gateway.client.clone(), &namespace)
        .replace(&name, &PostParams::default(), &updated)
        .await
        .map_err(|error| match error {
            kube::Error::Api(status) if status.code == 409 => (
                StatusCode::CONFLICT,
                "the server changed meanwhile, try again".to_owned(),
            ),
            error => {
                warn!("updating server failed: {error}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "updating the server failed".to_owned(),
                )
            }
        })?;
    info!(
        %namespace,
        server = %name,
        user = user.as_ref().map(|user| user.email.as_str()),
        "updated server"
    );
    let applied = if needs_restart(&server.spec, &updated.spec) {
        Applied::Restart
    } else {
        match apply_live(&gateway, &updated).await {
            Ok(()) => Applied::Live,
            Err(error) => {
                warn!(%namespace, server = %name, "applying settings right away failed: {error}");
                Applied::Later
            }
        }
    };
    Ok(Json(UpdateResult { applied }))
}

/// Whether a server has to be replaced for a change to its spec: a different mod, its content,
/// or running native code, which its pod is set up for.
fn needs_restart(old: &QuakeServerSpec, new: &QuakeServerSpec) -> bool {
    let fixed = |spec: &QuakeServerSpec| {
        let game = spec.game.as_ref();
        serde_json::to_value((
            game.and_then(|game| game.fs_game.as_ref()),
            game.and_then(|game| game.native),
            &spec.content,
        ))
        .unwrap_or_default()
    };
    fixed(old) != fixed(new)
}

/// Sends a server the config the controller will render for its new spec, so the change takes
/// effect without waiting for the config to reach the pod. The server checks it's from the
/// gateway by its rcon password, and ignores the config when it arrives later.
async fn apply_live(gateway: &Gateway, server: &QuakeServer) -> anyhow::Result<()> {
    let namespace = server.namespace().unwrap_or_default();
    let name = server.name_any();
    let secret = Api::<Secret>::namespaced(gateway.client.clone(), &namespace)
        .get(&name)
        .await?;
    let password = (secret.data.as_ref())
        .and_then(|data| data.get("rconPassword"))
        .map(|password| String::from_utf8_lossy(&password.0).into_owned())
        .ok_or_else(|| anyhow::anyhow!("the server's secret has no rcon password"))?;
    let config = server.render_config(gateway.master.as_deref(), &password)?;
    let response = gateway
        .http
        .post(format!("http://{name}.{namespace}.svc:{HTTP_PORT}/config"))
        .bearer_auth(password)
        .body(config)
        .timeout(Duration::from_secs(5))
        .send()
        .await?;
    if !response.status().is_success() {
        anyhow::bail!(
            "the server answered {}: {}",
            response.status(),
            response.text().await.unwrap_or_default().trim()
        );
    }
    Ok(())
}

/// A player on a server, as shown in the web app.
#[derive(Serialize)]
struct PlayerView {
    name: String,
    score: i32,
    ping: i32,
    bot: bool,
}

/// A running server's match, as shown in the web app.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct StatusView {
    ready: bool,
    /// The game it runs, so the play page notices when it changes.
    game: Option<String>,
    map: Option<String>,
    /// Best first.
    players: Vec<PlayerView>,
    frag_limit: Option<i32>,
    /// In minutes.
    time_limit: Option<i32>,
    capture_limit: Option<i32>,
}

/// Who's playing on a server, and to what limits.
async fn server_status(
    State(gateway): State<Arc<Gateway>>,
    Identity(_): Identity,
    Path((namespace, name)): Path<(String, String)>,
) -> Result<Json<StatusView>, StatusCode> {
    let server = gateway.server(&namespace, &name)?;
    let game = game_title(&gateway, &server);
    // Ready for its current spec: after switching games, the old server keeps running until
    // the controller has replaced it, and the play page mustn't rejoin that.
    let current = server.status.as_ref().is_some_and(|status| {
        status.ready && status.observed_generation == server.metadata.generation
    });
    if !current {
        return Ok(Json(StatusView {
            game,
            ..StatusView::default()
        }));
    }
    let query = async {
        let addr = registry::lookup(&format!("{name}.{namespace}.svc:{GAME_PORT}")).await?;
        let wait = Duration::from_secs(2);
        // The info says how many players are bots, which the status doesn't.
        let (status, info) = tokio::join!(
            protocol::get_status(addr, wait),
            protocol::get_info(addr, wait)
        );
        anyhow::Ok((status?, info.ok()))
    };
    let (status, info) = query.await.map_err(|error| {
        debug!(%namespace, server = %name, "querying players failed: {error}");
        StatusCode::BAD_GATEWAY
    })?;
    let bots = info.and_then(|info| {
        let number = |key: &str| info.get(key)?.parse::<usize>().ok();
        Some(number("clients")?.saturating_sub(number("g_humanplayers")?))
    });
    let bot_pings = is_bot_ping(&status.players, bots);
    let mut players: Vec<PlayerView> = status
        .players
        .into_iter()
        .map(|player| PlayerView {
            bot: bot_pings && player.ping == 0,
            name: player.name,
            score: player.score,
            ping: player.ping,
        })
        .collect();
    players.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.name.cmp(&b.name)));
    // 0 means no limit.
    let limit = |key: &str| {
        (status.info.get(key))
            .and_then(|value| value.parse().ok())
            .filter(|&limit: &i32| limit > 0)
    };
    Ok(Json(StatusView {
        ready: true,
        game,
        map: status.info.get("mapname").cloned(),
        frag_limit: limit("fraglimit"),
        time_limit: limit("timelimit"),
        capture_limit: limit("capturelimit"),
        players,
    }))
}

/// Whether a ping of 0 means a bot. ioquake3 reports bots' pings as 0, but a player close
/// enough to the server, e.g. on the same machine, can have one too. It does unless more
/// players have it than there are bots, when which are bots can't be told.
fn is_bot_ping(players: &[protocol::Player], bots: Option<usize>) -> bool {
    let zero = players.iter().filter(|player| player.ping == 0).count();
    bots.is_none_or(|bots| zero <= bots)
}

/// Serves a thumbnail of a map's levelshot for a preset, so menus can show the maps of games
/// a server doesn't run. Until the preset's thumbnails are made, it comes from a running
/// server with the preset's content, if there is one.
async fn preset_levelshot(
    State(gateway): State<Arc<Gateway>>,
    Identity(_): Identity,
    Path((preset, file)): Path<(String, String)>,
) -> Result<Response, StatusCode> {
    let path = format!("levelshots/{file}");
    if !proxied(&Method::GET, &path) {
        return Err(StatusCode::NOT_FOUND);
    }
    let map = file.trim_end_matches(".jpg");
    if let Some(data) = gateway.levelshots.get(&preset, map) {
        return Ok((
            [
                (header::CONTENT_TYPE, "image/jpeg"),
                (header::CACHE_CONTROL, "public, max-age=3600"),
            ],
            data,
        )
            .into_response());
    }
    let server = (gateway.store.state().into_iter())
        .find(|qs| {
            qs.labels().get(PRESET_LABEL) == Some(&preset)
                && qs.status.as_ref().is_some_and(|status| status.ready)
        })
        .ok_or(StatusCode::NOT_FOUND)?;
    let url = format!(
        "http://{}.{}.svc:{HTTP_PORT}/{path}",
        server.name_any(),
        server.namespace().unwrap_or_default()
    );
    let upstream = gateway.http.get(&url).send().await.map_err(|error| {
        debug!(%url, "fetching levelshot failed: {error}");
        StatusCode::BAD_GATEWAY
    })?;
    if !upstream.status().is_success() {
        return Err(StatusCode::NOT_FOUND);
    }
    let body = upstream
        .bytes()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/jpeg"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        body,
    )
        .into_response())
}

/// Serves the ioquake3 web client.
async fn client_file(State(gateway): State<Arc<Gateway>>, Path(file): Path<String>) -> Response {
    let Some(client) = &gateway.web_client else {
        return StatusCode::NOT_FOUND.into_response();
    };
    serve_client_file(&file, &client.js, &client.wasm)
}

/// Serves the ioquake3 web client that reads paks on demand.
async fn jspi_client_file(
    State(gateway): State<Arc<Gateway>>,
    Path(file): Path<String>,
) -> Response {
    let Some((js, wasm)) = gateway.web_client.as_ref().and_then(|c| c.jspi.as_ref()) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    serve_client_file(&file, js, wasm)
}

fn serve_client_file(file: &str, js: &Bytes, wasm: &Bytes) -> Response {
    let (content_type, body) = match file {
        "ioquake3.js" => ("text/javascript", js.clone()),
        "ioquake3.wasm" => ("application/wasm", wasm.clone()),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        body,
    )
        .into_response()
}

/// The page that runs the browser client for one server.
async fn play(
    State(gateway): State<Arc<Gateway>>,
    Identity(user): Identity,
    Path((namespace, name)): Path<(String, String)>,
) -> Result<Html<String>, StatusCode> {
    let server = gateway.server(&namespace, &name)?;
    // What the server's owner can change from the game's menu.
    let manage = authorize(&gateway, &server, user.as_ref(), &namespace)
        .await
        .is_ok()
        .then(|| {
            json!({
                "settings": ServerSettings::of(&server),
                "presets": preset_views(&gateway),
            })
        });
    let client = gateway
        .web_client
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let needpass = gateway
        .registry
        .get(&ObjectRef::from_obj(server.as_ref()))
        .and_then(|entry| entry.info.get("g_needpass").cloned());
    let needs_password = needs_password(&server, needpass.as_deref());
    // How the page reaches the WebTransport endpoint, if there is one.
    let webtransport = gateway.webtransport.as_ref().map(|wt| {
        json!({
            "address": wt.public_address(),
            "port": wt.public_port(),
            "certificateHashes": wt.certificate_hashes(),
            "ticket": gateway.auth.enabled().then(|| gateway.auth.ticket(&namespace, &name)),
        })
    });
    // Kubernetes names can't contain characters that need escaping in HTML or JavaScript, and
    // escaping < keeps the JSON from closing the script element.
    Ok(Html(
        PLAY_HTML
            .replace("{{NAMESPACE}}", &namespace)
            .replace("{{CLIENT_VERSION}}", &client.version)
            .replace(
                "{{ON_DEMAND}}",
                if client.jspi.is_some() {
                    "true"
                } else {
                    "false"
                },
            )
            .replace("{{NAME}}", &name)
            .replace(
                "{{WEBTRANSPORT}}",
                &serde_json::to_string(&webtransport)
                    .unwrap_or_else(|_| "null".to_owned())
                    .replace('<', "\\u003c"),
            )
            .replace(
                "{{NEEDS_PASSWORD}}",
                if needs_password { "true" } else { "false" },
            )
            .replace(
                "{{MAPS}}",
                &serde_json::to_string(
                    &server
                        .spec
                        .maps
                        .iter()
                        .map(|map| map.name.to_lowercase())
                        .collect::<Vec<_>>(),
                )
                .unwrap_or_else(|_| "[]".to_owned())
                .replace('<', "\\u003c"),
            )
            // Last, since these come from users.
            .replace(
                "{{MANAGE}}",
                &serde_json::to_string(&manage)
                    .unwrap_or_else(|_| "null".to_owned())
                    .replace('<', "\\u003c"),
            )
            .replace(
                "{{SUGGESTED_NAME}}",
                &serde_json::to_string(&user.map(|user| suggested_name(&user.email)))
                    .unwrap_or_else(|_| "null".to_owned())
                    .replace('<', "\\u003c"),
            ),
    ))
}

/// A player name for someone who hasn't chosen one, from their email address, e.g. "ranger"
/// for ranger+quake@example.com.
fn suggested_name(email: &str) -> String {
    let local = email.split('@').next().unwrap_or_default();
    let local = local.split('+').next().unwrap_or_default();
    // The most the join form allows.
    local.chars().take(32).collect()
}

/// Whether a path on a game server's pod is one browser clients use: the server's info, its
/// content's manifest and pk3 files, and its maps' levelshots.
fn proxied(method: &Method, path: &str) -> bool {
    let pak = |name: &str| {
        name.ends_with(".pk3")
            && name
                .split('/')
                .all(|elem| !elem.is_empty() && !elem.starts_with('.'))
    };
    if *method == Method::POST {
        // Batches of ranges from a pak, for browsers that load files on demand.
        return path.strip_prefix("paks/").is_some_and(pak);
    }
    if let Some(map) = path.strip_prefix("levelshots/") {
        return map.strip_suffix(".jpg").is_some_and(|map| {
            !map.is_empty()
                && map
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '+' | '-'))
        });
    }
    if let Some(index) = path.strip_prefix("paks/") {
        // Which files the engine reads, and where they are in each pak.
        return index == "profile.json" || index.strip_suffix(".json").is_some_and(pak);
    }
    let Some(asset) = path.strip_prefix("assets/") else {
        return path == "info";
    };
    asset == "manifest.json" || pak(asset)
}

/// Proxies the server info and content browser clients download from the game server's pod.
async fn proxy(
    State(gateway): State<Arc<Gateway>>,
    Identity(_): Identity,
    Path((namespace, name, path)): Path<(String, String, String)>,
    RawQuery(query): RawQuery,
    method: Method,
    body: Body,
) -> Result<Response, StatusCode> {
    gateway.server(&namespace, &name)?;
    if !proxied(&method, &path) {
        return Err(StatusCode::NOT_FOUND);
    }
    let query = query.map(|q| format!("?{q}")).unwrap_or_default();
    let url = format!("http://{name}.{namespace}.svc:{HTTP_PORT}/{path}{query}");
    let mut request = gateway.http.request(method.clone(), &url);
    if method == Method::POST {
        // A list of ranges, which the game server limits too.
        let body = axum::body::to_bytes(body, 1 << 20)
            .await
            .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE)?;
        request = request
            .header(header::CONTENT_TYPE, "application/json")
            .body(body);
    }
    let upstream = request.send().await.map_err(|error| {
        debug!(%url, "proxy failed: {error}");
        StatusCode::BAD_GATEWAY
    })?;

    let mut response = Response::builder().status(upstream.status());
    for name in [
        header::CONTENT_TYPE,
        header::CONTENT_LENGTH,
        header::CACHE_CONTROL,
        header::ETAG,
        header::LAST_MODIFIED,
    ] {
        if let Some(value) = upstream.headers().get(&name) {
            response = response.header(name, value);
        }
    }
    response
        .body(Body::from_stream(upstream.bytes_stream()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Relays a browser client's game traffic between its WebSocket and the server's UDP port.
async fn relay(
    State(gateway): State<Arc<Gateway>>,
    Identity(_): Identity,
    Path((namespace, name)): Path<(String, String)>,
    ws: WebSocketUpgrade,
) -> Response {
    if let Err(status) = gateway.server(&namespace, &name) {
        return status.into_response();
    }
    let target = format!("{name}.{namespace}.svc:{GAME_PORT}");
    // The browser client asks for the "binary" subprotocol and gives up if it isn't accepted.
    ws.protocols(["binary"])
        .on_upgrade(move |socket| async move {
            debug!(%target, "browser client connected");
            if let Err(error) = relay_traffic(socket, &target).await {
                debug!(%target, "browser client disconnected: {error}");
            }
        })
}

async fn relay_traffic(ws: WebSocket, target: &str) -> anyhow::Result<()> {
    let addr = registry::lookup(target).await?;
    let udp = UdpSocket::bind(if addr.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    })
    .await?;
    udp.connect(addr).await?;
    let (mut to_client, mut from_client) = ws.split();

    let upstream = async {
        while let Some(message) = from_client.next().await {
            match message? {
                Message::Binary(data) => {
                    udp.send(&data).await?;
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        anyhow::Ok(())
    };
    let downstream = async {
        let mut buf = vec![0u8; 65536];
        loop {
            let len = udp.recv(&mut buf).await?;
            to_client
                .send(Message::Binary(Bytes::copy_from_slice(&buf[..len])))
                .await?;
        }
        #[allow(unreachable_code)]
        anyhow::Ok(())
    };
    tokio::select! {
        res = upstream => res,
        res = downstream => res,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxies_only_content() {
        let (get, post) = (Method::GET, Method::POST);
        for path in [
            "info",
            "assets/manifest.json",
            "assets/q3ut4/123-zUrT43_004.pk3",
            "paks/q3ut4/zUrT43_004.pk3.json",
            "paks/profile.json",
            "levelshots/q3dm17.jpg",
            "levelshots/ut4_abbey.jpg",
        ] {
            assert!(proxied(&get, path), "{path}");
        }
        assert!(proxied(&post, "paks/q3ut4/zUrT43_004.pk3"));
        for (method, path) in [
            (&get, "metrics"),
            (&get, "assets/q3ut4/server.cfg"),
            (&get, "assets/../etc/passwd.pk3"),
            (&get, "paks/q3ut4/.hidden.pk3.json"),
            (&get, "paks/q3ut4/zUrT43_004.pk3"),
            (&post, "paks/q3ut4/zUrT43_004.pk3.json"),
            (&post, "paks/profile.json"),
            (&post, "info"),
            (&post, "assets/q3ut4/123-zUrT43_004.pk3"),
            (&get, "levelshots/q3dm17.tga"),
            (&get, "levelshots/../q3dm17.jpg"),
            (&get, "levelshots/.jpg"),
            (&post, "levelshots/q3dm17.jpg"),
        ] {
            assert!(!proxied(method, path), "{method} {path}");
        }
    }

    #[test]
    fn names_servers() {
        assert_eq!(display_name("  Friday \"Night\"\\ 🔥 "), "Friday Night 🔥");
        assert_eq!(resource_name("Friday Night!"), "friday-night");
        assert_eq!(resource_name("  --Team__Arena 2-- "), "team-arena-2");
        assert_eq!(resource_name("🔥🔥"), "server");
        assert_eq!(numbered_name("friday-night", 1), "friday-night");
        assert_eq!(numbered_name("friday-night", 2), "friday-night-2");
        let long = resource_name(&"a".repeat(60));
        assert_eq!(numbered_name(&long, 1).len(), MAX_RESOURCE_NAME);
        assert_eq!(numbered_name(&long, 12).len(), MAX_RESOURCE_NAME);
        assert!(numbered_name(&long, 12).ends_with("a-12"));
    }

    #[test]
    fn suggests_names() {
        assert_eq!(suggested_name("ranger@example.com"), "ranger");
        assert_eq!(suggested_name("ranger+quake@example.com"), "ranger");
        assert_eq!(
            suggested_name(&format!("{}@example.com", "x".repeat(40))).len(),
            32
        );
    }

    #[test]
    fn who_can_manage() {
        let user = |email: &str| User {
            email: email.to_owned(),
            groups: Vec::new(),
        };
        let (owner_user, other) = (user("ranger@example.com"), user("sarge@example.com"));
        let mut server = QuakeServer::new("duel", Default::default());
        // Not created from the web app.
        assert!(!manageable(&server, None, true));
        assert!(!manageable(&server, Some(&owner_user), true));
        server
            .labels_mut()
            .insert(CREATED_BY_LABEL.to_owned(), "gateway".to_owned());
        // Without auth, anyone can.
        assert!(manageable(&server, None, false));
        // Without an owner, e.g. created before auth was turned on, only admins can.
        assert!(!manageable(&server, Some(&owner_user), false));
        assert!(manageable(&server, Some(&owner_user), true));
        server
            .annotations_mut()
            .insert(OWNER_ANNOTATION.to_owned(), owner_user.email.clone());
        assert!(manageable(&server, Some(&owner_user), false));
        assert!(!manageable(&server, Some(&other), false));
        assert!(manageable(&server, Some(&other), true));
    }

    #[test]
    fn tells_bots_by_ping() {
        let players = |pings: &[i32]| -> Vec<protocol::Player> {
            (pings.iter())
                .map(|&ping| protocol::Player {
                    name: String::new(),
                    score: 0,
                    ping,
                })
                .collect()
        };
        assert!(is_bot_ping(&players(&[0, 0, 48]), Some(2)));
        // A player on the same machine as the server, besides two bots.
        assert!(!is_bot_ping(&players(&[0, 0, 0]), Some(2)));
        assert!(is_bot_ping(&players(&[0]), None));
    }

    #[test]
    fn restarts_for_another_game() {
        let spec = |value: serde_json::Value| -> QuakeServerSpec {
            serde_json::from_value(value).unwrap()
        };
        let base = spec(json!({ "game": { "fsGame": "cpma" }, "maps": [{ "name": "cpm22" }] }));
        let map = spec(json!({
            "game": { "fsGame": "cpma", "type": 1 },
            "maps": [{ "name": "cpm3a", "fragLimit": 20 }],
            "cvars": { "mode_start": "1v1" },
        }));
        assert!(!needs_restart(&base, &map));
        assert!(needs_restart(
            &base,
            &spec(json!({ "game": { "fsGame": "q3ut4" } }))
        ));
        assert!(needs_restart(
            &base,
            &spec(
                json!({ "game": { "fsGame": "cpma" }, "content": [{ "url": "https://example.com/a.zip" }] })
            ),
        ));
    }

    #[test]
    fn server_settings() {
        let preset: Preset = serde_json::from_value(json!({
            "name": "quake3",
            "title": "Quake 3",
            "gameTypes": [
                { "name": "Free For All", "type": "FreeForAll" },
                { "name": "Instagib", "type": "FreeForAll", "cvars": { "g_instagib": "1" } },
            ],
            "maps": ["q3dm17", "q3dm7"],
            "spec": { "server": { "maxClients": 8 } },
        }))
        .unwrap();
        let settings = ServerSettings {
            name: " Friday ".to_owned(),
            preset: "quake3".to_owned(),
            game_type: "Instagib".to_owned(),
            maps: vec!["q3dm17".to_owned(), "q3dm7".to_owned()],
            bots: 3,
            password: Some(String::new()),
            frag_limit: Some(30),
            time_limit: Some(15),
            capture_limit: Some(0),
            max_players: None,
        };
        let (game_type, hostname) = settings.validate(&preset).unwrap();
        assert_eq!(hostname, "Friday");
        let spec = settings.spec(&preset, game_type, &hostname);
        assert_eq!(spec.cvars["g_instagib"], "1");
        assert_eq!(spec.maps.len(), 2);
        assert_eq!(spec.maps[1].name, "q3dm7");
        assert_eq!(spec.maps[1].time_limit.as_deref(), Some("15m"));
        assert_eq!(spec.maps[0].capture_limit, None);
        // The preset's own setting stays unless one is chosen.
        assert_eq!(spec.server.as_ref().unwrap().max_clients, Some(8));
        assert_eq!(spec.game.as_ref().unwrap().password, None);

        // The settings read back from the server it makes.
        let mut server = QuakeServer::new("friday", spec);
        server
            .labels_mut()
            .insert(PRESET_LABEL.to_owned(), "quake3".to_owned());
        server
            .annotations_mut()
            .insert(GAME_TYPE_ANNOTATION.to_owned(), "Instagib".to_owned());
        assert_eq!(
            ServerSettings::of(&server),
            ServerSettings {
                name: "Friday".to_owned(),
                password: None,
                capture_limit: None,
                max_players: Some(8),
                ..settings.clone()
            }
        );

        for invalid in [
            ServerSettings {
                maps: vec!["q3dm17".to_owned(), "q3dm1".to_owned()],
                ..settings.clone()
            },
            ServerSettings {
                maps: Vec::new(),
                ..settings.clone()
            },
            ServerSettings {
                game_type: "CTF".to_owned(),
                ..settings.clone()
            },
            ServerSettings {
                frag_limit: Some(-1),
                ..settings.clone()
            },
            ServerSettings {
                max_players: Some(65),
                ..settings.clone()
            },
            ServerSettings {
                name: "\"\\".to_owned(),
                ..settings.clone()
            },
        ] {
            assert!(invalid.validate(&preset).is_err(), "{invalid:?}");
        }
    }
}
