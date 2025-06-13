use crate::controllers::resources::{ObjectRef, ObjectState, Objects};
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use kubera_core::select_continue;
use kubera_core::sync::signal::{channel, Receiver};
use tokio::task::JoinSet;

pub fn filter_gateways(
    join_set: &mut JoinSet<()>,
    gateway_classes: &Receiver<Objects<GatewayClass>>,
    gateways: &Receiver<Objects<Gateway>>,
) -> Receiver<Objects<Gateway>> {
    let (tx, rx) = channel(Objects::default());

    let mut gateway_classes = gateway_classes.clone();
    let mut gateways = gateways.clone();

    join_set.spawn(async move {
        loop {
            let current_gateway_classes = gateway_classes.current();
            let current_gateways = gateways.current();
            let filtered = current_gateways.filter(|_, gateway| {
                if let ObjectState::Active(gateway) = gateway {
                    let gateway_class_ref = ObjectRef::new_builder()
                        .of_kind::<GatewayClass>()
                        .namespace(None)
                        .name(&gateway.spec.gateway_class_name)
                        .build()
                        .unwrap();
                    current_gateway_classes.is_active(&gateway_class_ref)
                } else {
                    false
                }
            });

            tx.replace(filtered);

            select_continue!(gateway_classes.changed(), gateways.changed());
        }
    });

    rx
}
