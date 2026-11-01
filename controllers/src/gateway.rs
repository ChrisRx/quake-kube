//! The gateway serves the web app, which lists servers and lets people create them and play
//! in the browser. Browser clients can't send UDP, so their game traffic arrives over a
//! WebSocket and is relayed to the server here.

use crate::config::GatewayConfig;
use crate::prelude::*;
use crate::quake_server::MapSpec;
use crate::quake_server::{GAME_PORT, HTTP_PORT};
use crate::registry::{self, Registry};
use crate::webtransport::WebTransport;
use axum::body::{Body, Bytes};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, RawQuery, State};
use axum::http::{header, StatusCode};
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

const INDEX_HTML: &str = include_str!("../static/index.html");
const PLAY_HTML: &str = include_str!("../static/play.html");
const GAMESOCKET_JS: &str = include_str!("../static/gamesocket.js");
const EULA_TXT: &str = include_str!("../static/eula.txt");

/// The ioquake3 web client built into the image.
struct WebClient {
    /// Changes whenever the files do. Pages ask for the files by version, so they can be
    /// cached for good.
    version: String,
    js: Bytes,
    wasm: Bytes,
}

impl WebClient {
    fn load(dir: &std::path::Path) -> std::io::Result<Self> {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let js = std::fs::read(dir.join("ioquake3.js"))?;
        let wasm = std::fs::read(dir.join("ioquake3.wasm"))?;
        let mut hasher = DefaultHasher::new();
        js.hash(&mut hasher);
        wasm.hash(&mut hasher);
        Ok(WebClient {
            version: format!("{:016x}", hasher.finish()),
            js: js.into(),
            wasm: wasm.into(),
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

pub async fn run(client: Client, config: Config) -> anyhow::Result<()> {
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
        tokio::spawn(webtransport.clone().serve(store.clone()));
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
    });

    let app = Router::new()
        .route("/", get(|| async { Html(INDEX_HTML) }))
        .route("/healthz", get(|| async { "ok" }))
        .route(
            "/static/gamesocket.js",
            get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], GAMESOCKET_JS) }),
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
        .route("/api/config", get(app_config))
        .route("/api/servers", get(list_servers).post(create_server))
        .route(
            "/api/servers/{namespace}/{name}",
            axum::routing::delete(delete_server),
        )
        .route("/play/{namespace}/{name}", get(play))
        .route("/play/{namespace}/{name}/", get(play))
        .route("/play/{namespace}/{name}/ws", get(relay))
        .route("/play/{namespace}/{name}/{*path}", get(proxy))
        .with_state(gateway);

    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    info!("gateway listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
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

async fn app_config(State(gateway): State<Arc<Gateway>>) -> Response {
    let config = &gateway.config;
    Json(AppConfig {
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
    /// Created from the web app, so it can be deleted from it.
    deletable: bool,
}

async fn list_servers(State(gateway): State<Arc<Gateway>>) -> Json<Vec<ServerView>> {
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
                deletable: qs.labels().get(CREATED_BY_LABEL).map(String::as_str) == Some("gateway"),
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
    let created = gateway
        .store
        .state()
        .iter()
        .filter(|qs| qs.labels().contains_key(CREATED_BY_LABEL))
        .count();
    if created >= config.max_servers {
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
    Ok(StatusCode::CREATED)
}

async fn delete_server(
    State(gateway): State<Arc<Gateway>>,
    Path((namespace, name)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let server = gateway
        .server(&namespace, &name)
        .map_err(|status| (status, "no such server".to_owned()))?;
    if !server.labels().contains_key(CREATED_BY_LABEL) {
        return Err((
            StatusCode::FORBIDDEN,
            "only servers created here can be deleted here".to_owned(),
        ));
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
    Ok(StatusCode::NO_CONTENT)
}

/// Serves the ioquake3 web client.
async fn client_file(State(gateway): State<Arc<Gateway>>, Path(file): Path<String>) -> Response {
    let Some(client) = &gateway.web_client else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let (content_type, body) = match file.as_str() {
        "ioquake3.js" => ("text/javascript", client.js.clone()),
        "ioquake3.wasm" => ("application/wasm", client.wasm.clone()),
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
        })
    });
    // Kubernetes names can't contain characters that need escaping in HTML or JavaScript, and
    // escaping < keeps the JSON from closing the script element.
    Ok(Html(
        PLAY_HTML
            .replace("{{NAMESPACE}}", &namespace)
            .replace("{{CLIENT_VERSION}}", &client.version)
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
            ),
    ))
}

/// Whether a path on a game server's pod is one browser clients use: the server's info, and
/// its content's manifest and pk3 files.
fn proxied(path: &str) -> bool {
    let Some(asset) = path.strip_prefix("assets/") else {
        return path == "info";
    };
    asset == "manifest.json"
        || (asset.ends_with(".pk3")
            && asset
                .split('/')
                .all(|elem| !elem.is_empty() && !elem.starts_with('.')))
}

/// Proxies the server info and content browser clients download from the game server's pod.
async fn proxy(
    State(gateway): State<Arc<Gateway>>,
    Path((namespace, name, path)): Path<(String, String, String)>,
    RawQuery(query): RawQuery,
) -> Result<Response, StatusCode> {
    gateway.server(&namespace, &name)?;
    if !proxied(&path) {
        return Err(StatusCode::NOT_FOUND);
    }
    let query = query.map(|q| format!("?{q}")).unwrap_or_default();
    let url = format!("http://{name}.{namespace}.svc:{HTTP_PORT}/{path}{query}");
    let upstream = gateway.http.get(&url).send().await.map_err(|error| {
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
