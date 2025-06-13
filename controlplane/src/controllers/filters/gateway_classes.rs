use crate::constants::GATEWAY_CLASS_CONTROLLER_NAME;
use crate::controllers::resources::{ObjectState, Objects};
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use kubera_core::select_continue;
use kubera_core::sync::signal::{channel, Receiver};
use tokio::task::JoinSet;

pub fn filter_gateway_classes(
    join_set: &mut JoinSet<()>,
    gateway_classes: &Receiver<Objects<GatewayClass>>,
) -> Receiver<Objects<GatewayClass>> {
    let (tx, rx) = channel(Objects::default());

    let mut gateway_classes = gateway_classes.clone();

    join_set.spawn(async move {
        loop {
            let current = gateway_classes.current();
            let filtered = current.filter(|_, gateway_class| {
                if let ObjectState::Active(gateway_class) = gateway_class {
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
