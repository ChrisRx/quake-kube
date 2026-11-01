// Nightly clippy (0.1.64) considers Drop a side effect, see https://github.com/rust-lang/rust-clippy/issues/9608
#![allow(clippy::unnecessary_lazy_evaluations)]

use crate::prelude::*;
use anyhow::Result;
use std::io::IsTerminal;
use std::path::Path;
use tracing_subscriber::EnvFilter;

mod auth;
mod config;
mod error;
mod gateway;
mod master;
pub mod prelude;
mod protocol;
mod quake_server;
mod quake_server_controller;
mod registry;
mod webtransport;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let role = args.next();
    if role.as_deref() == Some("crdgen") {
        let dir = args.next().unwrap_or_else(|| ".".to_owned());
        return crdgen(Path::new(&dir));
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_ansi(std::io::stdout().is_terminal())
        .init();
    let config = Config::load()?;
    let client = Client::try_default().await?;
    match role.as_deref() {
        Some("gateway") => gateway::run(client, config).await,
        _ => quake_server_controller::run(client, config).await,
    }
}

/// Writes the CRD manifest to a file in dir. The Helm chart installs it, so the controller
/// doesn't need permission to manage CRDs.
fn crdgen(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let crd = QuakeServer::crd();
    let path = dir.join(format!("{}.yaml", crd.name_any()));
    let yaml = serde_yaml_ng::to_string(&crd)?;
    // Leave an unchanged file alone so Tilt doesn't reload the chart for nothing.
    if std::fs::read_to_string(&path).ok().as_deref() != Some(yaml.as_str()) {
        std::fs::write(&path, yaml)?;
    }
    Ok(())
}
