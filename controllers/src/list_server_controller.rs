use crate::list_server::{ListServer, LIST_SERVER_FINALIZER};
use crate::prelude::*;

#[derive(Clone)]
pub struct Context {
    pub client: Client,
}

pub struct ListServerController {}

impl ListServerController {
    pub async fn reconcile(
        list_server: Arc<ListServer>,
        context: Arc<Context>,
    ) -> Result<Action, Error> {
        let client: Client = context.client.clone();

        let namespace: String = match list_server.namespace() {
            Some(namespace) => namespace,
            None => {
                return Err(Error::UserInputError(
                    "Expected ListServer resource to be namespaced. Can't deploy to an unknown namespace."
                        .to_owned(),
                ));
            }
        };
        let list_server_api: Api<ListServer> = Api::namespaced(client.clone(), &namespace);

        finalizer(
            &list_server_api,
            LIST_SERVER_FINALIZER,
            list_server,
            |event| async {
                match event {
                    Finalizer::Apply(list_server) => list_server.reconcile(context.clone()).await,
                    Finalizer::Cleanup(list_server) => list_server.cleanup(context.clone()).await,
                }
            },
        )
        .await
        .map_err(|e| Error::FinalizerError(Box::new(e)))
    }

    pub async fn run(client: Client, ns: Option<String>) -> Result<(), kube::Error> {
        let list_server_api = match &ns {
            Some(ns) => Api::<ListServer>::namespaced(client.clone(), ns),
            None => Api::<ListServer>::all(client.clone()),
        };

        Controller::new(list_server_api, Default::default())
            .run(
                ListServerController::reconcile,
                error_policy,
                Arc::new(Context { client }),
            )
            .for_each(|_| futures::future::ready(()))
            .await;
        Ok(())
    }
}

fn error_policy(_object: Arc<ListServer>, error: &Error, _ctx: Arc<Context>) -> Action {
    println!("Error: {error}");
    Action::requeue(Duration::from_secs(1))
}
