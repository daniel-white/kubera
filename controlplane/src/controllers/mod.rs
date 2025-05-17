use kube::Client;
use tokio::{join, spawn};

//mod gateway;
mod gateway_class;
mod gateway_class_parameters;
mod state;

pub async fn run_controllers() {
    let client = Client::try_default()
        .await
        .expect("Failed to create Kubernetes client");

    let (gateway_class_controller, gateway_class_state_rx) = gateway_class::controller(&client);
    let (gateway_class_parameters_controller, mut gateway_class_parameters_state_rx) =
        gateway_class_parameters::controller(&client, &gateway_class_state_rx);
    
    let x = spawn(async move {
        loop {
            let gateway_class_state = gateway_class_parameters_state_rx.current();
            dbg!("current state:", gateway_class_state);

            gateway_class_parameters_state_rx.changed().await;
        }
    });

    let _ = join!(
        gateway_class_controller,
        gateway_class_parameters_controller,
        x
    );
}
