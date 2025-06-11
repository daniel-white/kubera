use crate::controllers::resources::{Ref, ResourceState, Resources};
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use kubera_core::select_continue;
use kubera_core::sync::signal::{Receiver, channel};
use tokio::task::JoinSet;

pub fn filter_gateways(
    join_set: &mut JoinSet<()>,
    gateway_classes: &Receiver<Resources<GatewayClass>>,
    gateways: &Receiver<Resources<Gateway>>,
) -> Receiver<Resources<Gateway>> {
    let (tx, rx) = channel(Resources::default());

    let mut gateway_classes = gateway_classes.clone();
    let mut gateways = gateways.clone();

    join_set.spawn(async move {
        loop {
            let current_gateway_classes = gateway_classes.current();
            let filtered = gateways.current().filter_into(|_, gateway| {
                if let ResourceState::Active(gateway) = gateway {
                    let gateway_class_ref = Ref::new_builder()
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
