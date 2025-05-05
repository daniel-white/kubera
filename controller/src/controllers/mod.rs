mod gateway;
mod gateway_class;
mod gateway_class_parameters;
mod state;

use crate::controllers::state::StateEvents;
use gateway_class::watch_gateway_class;
use kube::Client;
use tokio::sync::mpsc::channel;

pub async fn run_controllers() {
    let client = Client::try_default()
        .await
        .expect("Failed to create Kubernetes client");

    let (state_tx, state_rx) = channel::<StateEvents>(100);

    watch_gateway_class(&client, state_tx).await
}
