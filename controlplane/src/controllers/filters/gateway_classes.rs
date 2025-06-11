use crate::constants::GATEWAY_CLASS_CONTROLLER_NAME;
use crate::controllers::resources::{ResourceState, Resources};
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use kubera_core::select_continue;
use kubera_core::sync::signal::{channel, Receiver};
use tokio::task::JoinSet;

pub fn filter_gateway_classes(
    join_set: &mut JoinSet<()>,
    gateway_classes: &Receiver<Resources<GatewayClass>>,
) -> Receiver<Resources<GatewayClass>> {
    let (tx, rx) = channel(Resources::default());

    let mut gateway_classes = gateway_classes.clone();

    join_set.spawn(async move {
        loop {
            let current = gateway_classes.current();
            let filtered = current.filter_into(|_, gateway_class| {
                if let ResourceState::Active(gateway_class) = gateway_class {
                    gateway_class.spec.controller_name == GATEWAY_CLASS_CONTROLLER_NAME
                } else {
                    false
                }
            });

            tx.replace(filtered);

            select_continue!(gateway_classes.changed());
        }
    });

    rx
}
