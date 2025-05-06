mod gateway;
mod gateway_class;
mod gateway_class_parameters;
mod state;

use crate::controllers::gateway_class::{
    GatewayClassController, SubscribeToGatewayClassParametersRefChangeBuilder,
};
use crate::controllers::gateway_class_parameters::GatewayClassParametersController;
use actix::prelude::*;
use kube::Client;
pub fn run_controllers() {
    let mut system = System::new();

    let gcc_addr: Addr<GatewayClassController> = system.block_on(async {
        let client = Client::try_default()
            .await
            .expect("Failed to create Kubernetes client");

        Supervisor::start(move |_| GatewayClassController::new(client))
    });

    let gcpc_addr: Addr<GatewayClassParametersController> = system.block_on(async {
        let client = Client::try_default()
            .await
            .expect("Failed to create Kubernetes client");

        Supervisor::start(move |_| GatewayClassParametersController::new(client))
    });

    gcc_addr.do_send(
        SubscribeToGatewayClassParametersRefChangeBuilder::default()
            .recipient(gcpc_addr.recipient().downgrade())
            .build()
            .unwrap(),
    );

    let _ = system.run();
}
