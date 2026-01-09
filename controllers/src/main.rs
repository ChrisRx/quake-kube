// Nightly clippy (0.1.64) considers Drop a side effect, see https://github.com/rust-lang/rust-clippy/issues/9608
#![allow(clippy::unnecessary_lazy_evaluations)]

use crate::prelude::*;
use anyhow::Result;
use tracing::*;

mod controller;
mod error;
mod list_server;
mod list_server_controller;
pub mod prelude;
mod quake_server;
mod quake_server_controller;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let client = Client::try_default().await?;
    let ssapply = PatchParams::apply("quake-kube-controller").force();
    let crds: Api<CustomResourceDefinition> = Api::all(client.clone());

    info!(
        "Creating crd: {}",
        serde_yaml::to_string(&QuakeServer::crd())?
    );
    crds.patch(
        "quakeservers.quake.kube",
        &ssapply,
        &Patch::Apply(QuakeServer::crd()),
    )
    .await?;

    info!(
        "Creating crd: {}",
        serde_yaml::to_string(&ListServer::crd())?
    );
    crds.patch(
        "listservers.quake.kube",
        &ssapply,
        &Patch::Apply(ListServer::crd()),
    )
    .await?;

    let _ = controller::run(client, None).await;
    Ok(())
}
