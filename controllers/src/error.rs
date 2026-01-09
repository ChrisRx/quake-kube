#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("KubeError: {0}")]
    KubeError(#[from] kube::Error),

    #[error("Invalid CRD: {0}")]
    UserInputError(String),

    #[error("Finalizer Error: {0}")]
    FinalizerError(#[source] Box<kube::runtime::finalizer::Error<Error>>),

    #[error("Failed to create ConfigMap: {0}")]
    ConfigMapCreationFailed(#[source] kube::Error),

    #[error("Failed to create Service: {0}")]
    ServiceCreationFailed(#[source] kube::Error),

    #[error("MissingObjectKey: {0}")]
    MissingObjectKey(&'static str),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
