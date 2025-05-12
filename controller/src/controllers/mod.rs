use crate::controllers::gateway_class::GatewayClassController;
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

    let gateway_class_controller = GatewayClassController::new(&client);
    let gateway_class_parameters_controller =
        gateway_class_parameters::GatewayClassParametersController::new(
            client,
            gateway_class_controller.state_rx(),
        );

    let mut x_state = gateway_class_parameters_controller.state_rs();
    let x = spawn(async move {
        loop {
            let gateway_class_state = x_state.current();
            dbg!("current state:", gateway_class_state);

            x_state.changed().await;
        }
    });

    let _ = join!(
        gateway_class_controller.run(),
        gateway_class_parameters_controller.run(),
        x
    );
}
