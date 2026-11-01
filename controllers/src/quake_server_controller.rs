use crate::master;
use crate::prelude::*;
use crate::registry::Registry;
use anyhow::Context as _;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tracing::*;

pub struct Context {
    pub client: Client,
    pub config: Config,
    pub registry: Arc<Registry>,
    /// Held while allocating a port so concurrent reconciles can't pick the same one.
    pub port_lock: Mutex<()>,
}

/// Runs the controller and the master server until a shutdown signal arrives.
pub async fn run(client: Client, config: Config) -> anyhow::Result<()> {
    // Only cache the objects this controller manages.
    let owned =
        watcher::Config::default().labels("app.kubernetes.io/managed-by=quake-kube-controller");

    let controller = Controller::new(Api::<QuakeServer>::all(client.clone()), Default::default())
        .owns(Api::<Deployment>::all(client.clone()), owned.clone())
        .owns(Api::<Service>::all(client.clone()), owned);

    // The master lists servers from the controller's cache.
    let registry = Registry::new(controller.store());
    let socket = UdpSocket::bind(("0.0.0.0", config.master.port)).await?;
    info!("master server listening on {}", socket.local_addr()?);
    tokio::spawn(registry.clone().poll());
    let master = master::serve(socket, registry.clone());

    let controller = controller
        .shutdown_on_signal()
        .run(
            reconcile,
            error_policy,
            Arc::new(Context {
                client,
                config,
                registry,
                port_lock: Mutex::new(()),
            }),
        )
        .for_each(|res| async move {
            if let Err(error) = res {
                warn!("reconcile failed: {error}");
            }
        });

    tokio::select! {
        _ = controller => Ok(()),
        res = master => Ok(res.context("master server failed")?),
    }
}

async fn reconcile(quake_server: Arc<QuakeServer>, ctx: Arc<Context>) -> Result<Action> {
    quake_server.reconcile(ctx).await
}

fn error_policy(_object: Arc<QuakeServer>, _error: &Error, _ctx: Arc<Context>) -> Action {
    Action::requeue(Duration::from_secs(5))
}
