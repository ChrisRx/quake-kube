//! The gateway serves the web app, which lists servers and lets people create them and play
//! in the browser. Browser clients can't send UDP, so their game traffic arrives over a
//! WebSocket and is relayed to the server here.

use crate::auth::{Auth, AuthError, User};
use crate::config::GatewayConfig;
use crate::prelude::*;
use crate::quake_server::MapSpec;
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
use kube::api::PostParams;
use kube::runtime::reflector::{self, ObjectRef, Store};
use kube::runtime::WatchStreamExt;
use serde_json::json;
use tokio::net::{TcpListener, UdpSocket};
use tracing::*;

/// Label on servers created from the web app. Only these can be deleted from it.
const CREATED_BY_LABEL: &str = "quake.kube/created-by";

/// Label with the preset a server was created from.
const PRESET_LABEL: &str = "quake.kube/preset";

/// Annotation with the email address of whoever created a server from the web app, when the
/// gateway knows who people are.
const OWNER_ANNOTATION: &str = "quake.kube/owner";

const INDEX_HTML: &str = include_str!("../static/index.html");
const PLAY_HTML: &str = include_str!("../static/play.html");
const GAMESOCKET_JS: &str = include_str!("../static/gamesocket.js");
const PAKS_JS: &str = include_str!("../static/paks.js");
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

/// Whether a server can be deleted from the web app by the user: one created there, by them
/// unless they're an admin.
fn deletable(server: &QuakeServer, user: Option<&User>, admin: bool) -> bool {
    if server.labels().get(CREATED_BY_LABEL).map(String::as_str) != Some("gateway") {
        return false;
    }
    match user {
        None => true,
        Some(user) => admin || owner(server) == Some(user.email.as_str()),
    }
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
    let gateway = Arc::new(Gateway {
        client,
        config: config.gateway,
        store,
        registry,
        http: reqwest::Client::new(),
        webtransport,
        web_client,
        auth,
    });

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
            axum::routing::delete(delete_server),
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
    presets: Vec<PresetView<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PresetView<'a> {
    name: &'a str,
    title: &'a str,
    game_types: Vec<&'a str>,
    maps: &'a [String],
    bots: bool,
}

async fn app_config(State(gateway): State<Arc<Gateway>>, Identity(user): Identity) -> Response {
    let config = &gateway.config;
    Json(AppConfig {
        user: user.map(|user| user.email),
        can_create: config.max_servers > 0 && !config.presets.is_empty(),
        presets: config
            .presets
            .iter()
            .map(|preset| PresetView {
                name: &preset.name,
                title: &preset.title,
                game_types: preset.game_types.iter().map(|t| t.name.as_str()).collect(),
                maps: &preset.maps,
                bots: preset.bots,
            })
            .collect(),
    })
    .into_response()
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
    game_type: Option<&'static str>,
    humans: Option<i32>,
    bots: Option<i32>,
    max_players: Option<i32>,
    needs_password: bool,
    /// Who created it from the web app, when the gateway knows who people are.
    owner: Option<String>,
    /// Whether the user can delete it from the web app.
    deletable: bool,
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
    let mut servers: Vec<ServerView> = gateway
        .store
        .state()
        .iter()
        .map(|qs| {
            let entry = gateway.registry.get(&ObjectRef::from_obj(qs.as_ref()));
            let info = entry.as_ref().map(|e| &e.info);
            let get = |key: &str| info.and_then(|info| info.get(key)).cloned();
            let number = |key: &str| get(key).and_then(|v| v.parse::<i32>().ok());
            let humans = number("g_humanplayers");
            let status = qs.status.clone().unwrap_or_default();
            let preset = qs.labels().get(PRESET_LABEL).and_then(|name| {
                gateway
                    .config
                    .presets
                    .iter()
                    .find(|preset| &preset.name == name)
            });
            ServerView {
                namespace: qs.namespace().unwrap_or_default(),
                name: qs.name_any(),
                game: preset
                    .map(|preset| preset.title.clone())
                    .or_else(|| qs.spec.game.as_ref().and_then(|game| game.fs_game.clone())),
                ready: status.ready,
                address: status.address,
                hostname: get("hostname"),
                map: get("mapname"),
                game_type: number("gametype")
                    .and_then(|n| game_type_name(get("game").as_deref(), n)),
                humans,
                bots: number("clients")
                    .zip(humans)
                    .map(|(all, humans)| all - humans),
                max_players: number("sv_maxclients"),
                needs_password: needs_password(qs, get("g_needpass").as_deref()),
                owner: owner(qs).map(String::from),
                deletable: deletable(
                    qs,
                    user.as_ref(),
                    admin && qs.namespace().as_deref() == Some(gateway.config.namespace.as_str()),
                ),
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateServer {
    name: String,
    preset: String,
    game_type: String,
    map: String,
    #[serde(default)]
    bots: i32,
    password: Option<String>,
}

async fn create_server(
    State(gateway): State<Arc<Gateway>>,
    Identity(user): Identity,
    Json(request): Json<CreateServer>,
) -> Result<StatusCode, (StatusCode, String)> {
    let config = &gateway.config;
    let bad_request = |msg: &str| Err((StatusCode::BAD_REQUEST, msg.to_owned()));
    if config.max_servers == 0 {
        return Err((
            StatusCode::FORBIDDEN,
            "creating servers is turned off".to_owned(),
        ));
    }
    // Leave room for the "-game" suffix of the Service name.
    let valid_name = (1..=40).contains(&request.name.len())
        && request
            .name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !request.name.starts_with('-')
        && !request.name.ends_with('-');
    if !valid_name {
        return bad_request("names use lowercase letters, digits and dashes, up to 40 characters");
    }
    let Some(preset) = config.presets.iter().find(|p| p.name == request.preset) else {
        return bad_request("unknown game");
    };
    let Some(game_type) = preset
        .game_types
        .iter()
        .find(|t| t.name == request.game_type)
    else {
        return bad_request("unknown game type");
    };
    if !preset.maps.contains(&request.map) {
        return bad_request("unknown map");
    }
    if !(0..=16).contains(&request.bots) {
        return bad_request("bots must be between 0 and 16");
    }
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

    let mut spec = preset.spec.clone();
    if preset.bots {
        spec.bot.get_or_insert_default().min_players = Some(request.bots);
    }
    let game = spec.game.get_or_insert_default();
    if game_type.type_.is_some() {
        game.type_ = game_type.type_;
    }
    game.password = request.password.filter(|p| !p.is_empty());
    spec.cvars.extend(game_type.cvars.clone());
    spec.server.get_or_insert_default().hostname = Some(request.name.clone());
    spec.maps = vec![MapSpec {
        name: request.map,
        type_: None,
        capture_limit: None,
        frag_limit: None,
        time_limit: None,
    }];
    let mut server = QuakeServer::new(&request.name, spec);
    let labels = server.labels_mut();
    labels.insert(CREATED_BY_LABEL.to_owned(), "gateway".to_owned());
    labels.insert(PRESET_LABEL.to_owned(), preset.name.clone());
    if let Some(user) = &user {
        server
            .annotations_mut()
            .insert(OWNER_ANNOTATION.to_owned(), user.email.clone());
    }
    Api::<QuakeServer>::namespaced(gateway.client.clone(), &config.namespace)
        .create(&PostParams::default(), &server)
        .await
        .map_err(|error| match error {
            kube::Error::Api(status) if status.code == 409 => (
                StatusCode::CONFLICT,
                format!("{} already exists", request.name),
            ),
            error => {
                warn!("creating server failed: {error}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "creating the server failed".to_owned(),
                )
            }
        })?;
    info!(
        server = %request.name,
        user = user.as_ref().map(|user| user.email.as_str()),
        "created server"
    );
    Ok(StatusCode::CREATED)
}

async fn delete_server(
    State(gateway): State<Arc<Gateway>>,
    Identity(user): Identity,
    Path((namespace, name)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let server = gateway
        .server(&namespace, &name)
        .map_err(|status| (status, "no such server".to_owned()))?;
    if !deletable(&server, user.as_ref(), false) {
        // Only ask the API server about admins when it matters.
        let admin = match &user {
            Some(user) => gateway.auth.is_admin(user, &namespace).await,
            None => false,
        };
        if !deletable(&server, user.as_ref(), admin) {
            return Err((
                StatusCode::FORBIDDEN,
                "only servers created here can be deleted, by whoever created them or an admin"
                    .to_owned(),
            ));
        }
    }
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
            // Last, since it comes from the user.
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

/// Whether a path on a game server's pod is one browser clients use: the server's info, and
/// its content's manifest and pk3 files.
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
        ] {
            assert!(!proxied(method, path), "{method} {path}");
        }
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
    fn who_can_delete() {
        let user = |email: &str| User {
            email: email.to_owned(),
            groups: Vec::new(),
        };
        let (owner_user, other) = (user("ranger@example.com"), user("sarge@example.com"));
        let mut server = QuakeServer::new("duel", Default::default());
        // Not created from the web app.
        assert!(!deletable(&server, None, true));
        assert!(!deletable(&server, Some(&owner_user), true));
        server
            .labels_mut()
            .insert(CREATED_BY_LABEL.to_owned(), "gateway".to_owned());
        // Without auth, anyone can.
        assert!(deletable(&server, None, false));
        // Without an owner, e.g. created before auth was turned on, only admins can.
        assert!(!deletable(&server, Some(&owner_user), false));
        assert!(deletable(&server, Some(&owner_user), true));
        server
            .annotations_mut()
            .insert(OWNER_ANNOTATION.to_owned(), owner_user.email.clone());
        assert!(deletable(&server, Some(&owner_user), false));
        assert!(!deletable(&server, Some(&other), false));
        assert!(deletable(&server, Some(&other), true));
    }
}
