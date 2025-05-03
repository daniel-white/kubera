pub mod gateway_class;

use gateway_class::watch_gateway_class;
use kube::Client;

pub async fn run_controllers() {
    let client = Client::try_default()
        .await
        .expect("Failed to create Kubernetes client");

    watch_gateway_class(&client).await
}
