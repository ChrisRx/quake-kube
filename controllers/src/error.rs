use crate::config::PortRange;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("KubeError: {0}")]
    KubeError(#[from] kube::Error),

    #[error("MissingObjectKey: {0}")]
    MissingObjectKey(&'static str),

    #[error("Failed to render server config: {0}")]
    RenderJson(#[from] serde_json::Error),

    #[error("Failed to render server config: {0}")]
    RenderYaml(#[from] serde_yaml_ng::Error),

    #[error("no free ports left in {0}")]
    PortsExhausted(PortRange),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
