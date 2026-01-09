use crate::list_server_controller::Context;
use crate::prelude::*;

pub const LIST_SERVER_FINALIZER: &str = "listservers.quake.kube/finalizer";

#[derive(CustomResource, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[kube(group = "quake.kube", version = "v1", kind = "ListServer")]
#[kube(shortname = "ls", namespaced)]
pub struct ListServerSpec {
    #[serde(default = "ListServer::default_port")]
    pub port: i32,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub struct ListServerStatus {
    pub ready: bool,
}

impl ListServer {
    fn default_port() -> i32 {
        27950
    }

    pub async fn reconcile(&self, ctx: Arc<Context>) -> Result<Action> {
        let client: Client = ctx.client.clone();

        let namespace: String = match self.namespace() {
            Some(namespace) => namespace,
            None => {
                return Err(Error::UserInputError(
                    "Expected ListServer resource to be namespaced. Can't deploy to an unknown namespace."
                        .to_owned(),
                ));
            }
        };

        // TODO(chrism): Need to actually handle updating status here. Also need to do a backoff
        // for requeues depending upon status. If status is good it can requeue for like 10m, and
        // depending upon error should probably do an exponential backoff.

        self.deploy_service(ctx.clone()).await?;
        self.deploy(client, self.name_any().as_str(), 1, &namespace)
            .await?;
        Ok(Action::requeue(Duration::from_secs(10)))
    }

    pub async fn cleanup(&self, ctx: Arc<Context>) -> Result<Action> {
        let client: Client = ctx.client.clone();

        let namespace: String = match self.namespace() {
            Some(namespace) => namespace,
            None => {
                return Err(Error::UserInputError(
                    "Expected ListServer resource to be namespaced. Can't deploy to an unknown namespace."
                        .to_owned(),
                ));
            }
        };

        self.delete(client.clone(), self.name_any().as_str(), &namespace)
            .await?;
        Ok(Action::await_change())
    }

    async fn deploy_service(&self, ctx: Arc<Context>) -> Result<Service, Error> {
        let client: Client = ctx.client.clone();

        let mut selector = BTreeMap::new();
        selector.insert("app".to_string(), self.name_any());
        let oref = self.controller_owner_ref(&()).unwrap();
        let svc = Service {
            metadata: ObjectMeta {
                name: self.metadata.name.clone(),
                owner_references: Some(vec![oref]),
                ..ObjectMeta::default()
            },
            spec: Some(ServiceSpec {
                type_: Some("NodePort".to_owned()),
                selector: Some(selector),
                ports: Some(vec![ServicePort {
                    name: Some("server".to_owned()),
                    port: self.spec.port,
                    node_port: Some(30005),
                    protocol: Some("UDP".to_string()),
                    target_port: Some(IntOrString::Int(self.spec.port)),
                    ..ServicePort::default()
                }]),
                ..ServiceSpec::default()
            }),
            ..Default::default()
        };
        let svc_api = Api::<Service>::namespaced(
            client.clone(),
            self.metadata
                .namespace
                .as_ref()
                .ok_or_else(|| Error::MissingObjectKey(".metadata.namespace"))?,
        );

        svc_api
            .patch(
                svc.metadata
                    .name
                    .as_ref()
                    .ok_or_else(|| Error::MissingObjectKey(".metadata.name"))?,
                &PatchParams::apply("listservers.quake.kube"),
                &Patch::Apply(&svc),
            )
            .await
            .map_err(Error::ServiceCreationFailed)
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
                            name: "server".to_owned(),
                            image: Some("ghcr.io/chrisrx/dpmaster".to_owned()),
                            ports: Some(vec![ContainerPort {
                                container_port: 27950,
                                ..ContainerPort::default()
                            }]),
                            ..Container::default()
                        }],
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
