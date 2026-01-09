use crate::prelude::*;
use crate::quake_server::{QuakeServer, QUAKE_SERVER_FINALIZER};

#[derive(Clone)]
pub struct Context {
    pub client: Client,
}

pub struct QuakeServerController {}

impl QuakeServerController {
    pub async fn reconcile(
        quake_server: Arc<QuakeServer>,
        context: Arc<Context>,
    ) -> Result<Action, Error> {
        let client: Client = context.client.clone();

        let namespace: String = match quake_server.namespace() {
            Some(namespace) => namespace,
            None => {
                return Err(Error::UserInputError(
                    "Expected QuakeServer resource to be namespaced. Can't deploy to an unknown namespace."
                        .to_owned(),
                ));
            }
        };
        let quake_server_api: Api<QuakeServer> = Api::namespaced(client.clone(), &namespace);

        finalizer(
            &quake_server_api,
            QUAKE_SERVER_FINALIZER,
            quake_server,
            |event| async {
                match event {
                    Finalizer::Apply(quake_server) => quake_server.reconcile(context.clone()).await,
                    Finalizer::Cleanup(quake_server) => quake_server.cleanup(context.clone()).await,
                }
            },
        )
        .await
        .map_err(|e| Error::FinalizerError(Box::new(e)))
    }

    pub async fn run(client: Client, ns: Option<String>) -> Result<(), kube::Error> {
        let quake_server_api = match &ns {
            Some(ns) => Api::<QuakeServer>::namespaced(client.clone(), ns),
            None => Api::<QuakeServer>::all(client.clone()),
        };

        Controller::new(quake_server_api, Default::default())
            .run(
                QuakeServerController::reconcile,
                error_policy,
                Arc::new(Context { client }),
            )
            .for_each(|_| futures::future::ready(()))
            .await;
        Ok(())
    }
}

fn error_policy(_object: Arc<QuakeServer>, error: &Error, _ctx: Arc<Context>) -> Action {
    println!("Error: {error}");
    Action::requeue(Duration::from_secs(1))
}
