use crate::objects::{ObjectRef, ObjectState, Objects};
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use kubera_core::continue_on;
use kubera_core::sync::signal::{channel, Receiver};
use tokio::spawn;

pub fn filter_gateways(
    gateway_classes: &Receiver<Objects<GatewayClass>>,
    gateways: &Receiver<Objects<Gateway>>,
) -> Receiver<Objects<Gateway>> {
    let (tx, rx) = channel(Objects::default());

    let mut gateway_classes = gateway_classes.clone();
    let mut gateways = gateways.clone();

    spawn(async move {
        loop {
            let current_gateway_classes = gateway_classes.current();
            let current_gateways = gateways.current();
            let filtered = current_gateways
                .iter()
                .filter(|(_, _, gateway)| {
                    if let ObjectState::Active(gateway) = gateway {
                        let gateway_class_ref = ObjectRef::new_builder()
                            .of_kind::<GatewayClass>()
                            .namespace(None)
                            .name(&gateway.spec.gateway_class_name)
                            .build()
                            .unwrap();
                        current_gateway_classes
                            .get_by_ref(&gateway_class_ref)
                            .map(|o| o.is_active())
                            .unwrap_or_default()
                    } else {
                        false
                    }
                })
                .collect();

            tx.replace(filtered);

            continue_on!(gateway_classes.changed(), gateways.changed());
        }
    });

    rx
}
