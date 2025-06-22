use crate::objects::{ObjectState, Objects};
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use kubera_api::constants::GATEWAY_CLASS_CONTROLLER_NAME;
use kubera_core::continue_on;
use kubera_core::sync::signal::{channel, Receiver};
use tokio::spawn;

pub fn filter_gateway_classes(
    gateway_classes: &Receiver<Objects<GatewayClass>>,
) -> Receiver<Objects<GatewayClass>> {
    let (tx, rx) = channel(Objects::default());

    let mut gateway_classes = gateway_classes.clone();

    spawn(async move {
        loop {
            let current = gateway_classes.current();
            let filtered: Objects<_> = current
                .iter()
                .filter(|(_, _, gateway_class)| {
                    if let ObjectState::Active(gateway_class) = gateway_class {
                        gateway_class.spec.controller_name == GATEWAY_CLASS_CONTROLLER_NAME
                    } else {
                        false
                    }
                })
                .collect();

            tx.replace(filtered);

            continue_on!(gateway_classes.changed());
        }
    });

    rx
}
