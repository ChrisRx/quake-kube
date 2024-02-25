use crate::error::{Error, Result};
use crate::quake_server_controller::Context;
use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec};
use k8s_openapi::api::core::v1::{ConfigMap, EmptyDirVolumeSource, ExecAction};
use k8s_openapi::api::core::v1::{
    ConfigMapVolumeSource, Container, ContainerPort, PodSpec, PodTemplateSpec, Probe, Volume,
    VolumeMount,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use kube::{
    api::{Api, DeleteParams, ObjectMeta, Patch, PatchParams, PostParams, ResourceExt},
    client::Client,
    runtime::controller::Action,
    CustomResource, Resource,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::time::Duration;

pub const QUAKE_SERVER_FINALIZER: &str = "quakeservers.quake.kube/finalizer";

#[derive(CustomResource, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[kube(group = "quake.kube", version = "v1", kind = "QuakeServer")]
#[kube(shortname = "qs", namespaced)]
pub struct QuakeServerSpec {
    pub config: String,
}

/// The status object of `Document`
#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub struct QuakeServerStatus {
    pub hidden: bool,
}

impl QuakeServer {
    pub async fn reconcile(&self, ctx: Arc<Context>) -> Result<Action> {
        let client: Client = ctx.client.clone();

        let namespace: String = match self.namespace() {
            Some(namespace) => namespace,
            None => {
                return Err(Error::UserInputError(
                    "Expected QuakeServer resource to be namespaced. Can't deploy to an unknown namespace."
                        .to_owned(),
                ));
            }
        };
        self.deploy_configmap(ctx.clone()).await?;
        self.deploy(client, self.name_any().as_str(), 1, &namespace)
            .await?;
        Ok(Action::requeue(Duration::from_secs(10)))
    }

    async fn deploy_configmap(&self, ctx: Arc<Context>) -> Result<ConfigMap, Error> {
        let client: Client = ctx.client.clone();

        let mut contents = BTreeMap::new();
        contents.insert("config.yaml".to_string(), self.spec.config.clone());
        let oref = self.controller_owner_ref(&()).unwrap();
        let cm = ConfigMap {
            metadata: ObjectMeta {
                name: self.metadata.name.clone(),
                owner_references: Some(vec![oref]),
                ..ObjectMeta::default()
            },
            data: Some(contents),
            ..Default::default()
        };
        let cm_api = Api::<ConfigMap>::namespaced(
            client.clone(),
            self.metadata
                .namespace
                .as_ref()
                .ok_or_else(|| Error::MissingObjectKey(".metadata.namespace"))?,
        );
        cm_api
            .patch(
                cm.metadata
                    .name
                    .as_ref()
                    .ok_or_else(|| Error::MissingObjectKey(".metadata.name"))?,
                &PatchParams::apply("quakeservers.quake.kube"),
                &Patch::Apply(&cm),
            )
            .await
            .map_err(Error::ConfigMapCreationFailed)
    }

    pub async fn cleanup(&self, ctx: Arc<Context>) -> Result<Action> {
        let client: Client = ctx.client.clone();

        let namespace: String = match self.namespace() {
            Some(namespace) => namespace,
            None => {
                return Err(Error::UserInputError(
                    "Expected QuakeServer resource to be namespaced. Can't deploy to an unknown namespace."
                        .to_owned(),
                ));
            }
        };

        self.delete(client.clone(), self.name_any().as_str(), &namespace)
            .await?;
        Ok(Action::await_change())
    }

    async fn deploy(
        &self,
        client: Client,
        name: &str,
        replicas: i32,
        namespace: &str,
    ) -> Result<(), kube::Error> {
        let mut labels: BTreeMap<String, String> = BTreeMap::new();
        labels.insert("app".to_owned(), name.to_owned());

        let deployment_api: Api<Deployment> = Api::namespaced(client, namespace);
        if deployment_api.get_opt(name).await.unwrap().is_some() {
            return Ok(());
        }

        // Definition of the deployment. Alternatively, a YAML representation could be used as well.
        let deployment: Deployment = Deployment {
            metadata: ObjectMeta {
                name: Some(name.to_owned()),
                namespace: Some(namespace.to_owned()),
                labels: Some(labels.clone()),
                ..ObjectMeta::default()
            },
            spec: Some(DeploymentSpec {
                replicas: Some(replicas),
                selector: LabelSelector {
                    match_expressions: None,
                    match_labels: Some(labels.clone()),
                },
                template: PodTemplateSpec {
                    spec: Some(PodSpec {
                        containers: vec![Container {
                            command: Some(
                                [
                                    "q3",
                                    "run",
                                    "--config=/config/config.yaml",
                                    "--agree-eula",
                                    "--shutdown-delay=10s",
                                ]
                                .iter()
                                .map(|&s| s.into())
                                .collect(),
                            ),
                            name: "server".to_owned(),
                            image: Some("ghcr.io/chrisrx/quake-kube:latest".to_owned()),
                            ports: Some(vec![ContainerPort {
                                container_port: 8080,
                                ..ContainerPort::default()
                            }]),
                            liveness_probe: Some(Probe {
                                exec: Some(ExecAction {
                                    command: Some(
                                        ["grpc-health-probe", "-addr=localhost:8080"]
                                            .iter()
                                            .map(|&s| s.into())
                                            .collect(),
                                    ),
                                }),
                                initial_delay_seconds: Some(30),
                                failure_threshold: Some(1),
                                success_threshold: Some(1),
                                period_seconds: Some(10),
                                ..Probe::default()
                            }),
                            readiness_probe: Some(Probe {
                                exec: Some(ExecAction {
                                    command: Some(
                                        ["grpc-health-probe", "-addr=localhost:8080"]
                                            .iter()
                                            .map(|&s| s.into())
                                            .collect(),
                                    ),
                                }),
                                initial_delay_seconds: Some(5),
                                failure_threshold: Some(3),
                                success_threshold: Some(1),
                                period_seconds: Some(3),
                                ..Probe::default()
                            }),
                            volume_mounts: Some(vec![
                                VolumeMount {
                                    name: name.to_string(),
                                    mount_path: "/config".to_string(),
                                    ..VolumeMount::default()
                                },
                                VolumeMount {
                                    name: "quake3-content".to_string(),
                                    mount_path: "/assets".to_string(),
                                    ..VolumeMount::default()
                                },
                            ]),
                            ..Container::default()
                        }],
                        volumes: Some(vec![
                            Volume {
                                name: name.to_string(),
                                config_map: Some(ConfigMapVolumeSource {
                                    name: Some(name.to_string()),
                                    ..ConfigMapVolumeSource::default()
                                }),
                                ..Volume::default()
                            },
                            Volume {
                                name: "quake3-content".to_string(),
                                empty_dir: Some(EmptyDirVolumeSource {
                                    ..EmptyDirVolumeSource::default()
                                }),
                                ..Volume::default()
                            },
                        ]),
                        ..PodSpec::default()
                    }),
                    metadata: Some(ObjectMeta {
                        labels: Some(labels),
                        ..ObjectMeta::default()
                    }),
                },
                ..DeploymentSpec::default()
            }),
            ..Deployment::default()
        };

        // Create the deployment defined above
        deployment_api
            .create(&PostParams::default(), &deployment)
            .await?;
        Ok(())
    }

    async fn delete(&self, client: Client, name: &str, namespace: &str) -> Result<(), kube::Error> {
        let api: Api<Deployment> = Api::namespaced(client, namespace);
        api.delete(name, &DeleteParams::default()).await?;
        Ok(())
    }
}
