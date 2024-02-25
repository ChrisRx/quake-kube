// Nightly clippy (0.1.64) considers Drop a side effect, see https://github.com/rust-lang/rust-clippy/issues/9608
#![allow(clippy::unnecessary_lazy_evaluations)]

use anyhow::Result;
use apiexts::CustomResourceDefinition;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1 as apiexts;
use kube::{
    api::{Api, Patch, PatchParams},
    Client, CustomResourceExt,
};
use tracing::*;

mod controller;
mod error;
mod quake_server;
use crate::quake_server::QuakeServer;
mod list_server;
mod list_server_controller;
mod quake_server_controller;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let client = Client::try_default().await?;

    let ssapply = PatchParams::apply("crd_apply_example").force();

    // 0. Ensure the CRD is installed (you probably just want to do this on CI)
    // (crd file can be created by piping `Foo::crd`'s yaml ser to kubectl apply)
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

    let _ = controller::run(client, None).await;
    Ok(())
}
