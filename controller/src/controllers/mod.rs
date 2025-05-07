use kube::Client;
use ractor::Actor;
use crate::controllers::gateway_class::GatewayClassController;

mod gateway;
mod gateway_class;
//mod gateway_class_parameters;
mod state;

pub async fn run_controllers() {
    let client = Client::try_default()
        .await
        .expect("Failed to create Kubernetes client");
    
    let (actor, actor_handle) =
        Actor::spawn(None, GatewayClassController::new(&client), ())
            .await
            .expect("Actor failed to start");
}
