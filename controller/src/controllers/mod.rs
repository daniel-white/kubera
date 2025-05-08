use crate::controllers::gateway_class::GatewayClassController;
use futures_signals::signal::SignalExt;
use kube::Client;
use tokio::join;

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
        gateway_class_parameters::GatewayClassParametersController::new(client, gateway_class_controller.state());
    
    let x = gateway_class_parameters_controller.state().signal_cloned().for_each(|state| {
        if let Some(state) = state {
            println!("GatewayClassParametersState: {:?}", state);
        }
        async {}
    });


    let _ = join!(gateway_class_controller.run(), gateway_class_parameters_controller.run(), x);
}
