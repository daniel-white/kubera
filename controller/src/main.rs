use controllers::run_controllers;
use kube::Client;

mod controllers;

#[tokio::main]
async fn main() {
    let client = Client::try_default()
        .await
        .expect("Failed to create Kubernetes client");

    run_controllers(&client)
        .await
        .expect("Failed to run controller");
}
