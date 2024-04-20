use crate::list_server_controller::ListServerController;
use crate::quake_server_controller::QuakeServerController;
use kube::Client;
use tokio::task::JoinSet;

pub async fn run(client: Client, namespace: Option<String>) -> Result<(), kube::Error> {
    let mut controllers = JoinSet::new();
    controllers.spawn(QuakeServerController::run(
        client.clone(),
        namespace.clone(),
    ));
    controllers.spawn(ListServerController::run(client.clone(), namespace.clone()));

    while let Some(res) = controllers.join_next().await {
        println!("yeah");
        res.unwrap()?;
    }
    Ok(())
}
