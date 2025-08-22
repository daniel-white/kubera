use crate::kubernetes::objects::{ObjectRef, Objects};
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;
use vg_api::v1alpha1::GatewayParameters;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

pub fn filter_gateways(
    task_builder: &TaskBuilder,
    gateway_class_tx: &Receiver<(ObjectRef, Arc<GatewayClass>)>,
    gateways_rx: &Receiver<Objects<Gateway>>,
) -> Receiver<Objects<Gateway>> {
    let (tx, rx) = signal("filtered_gateways");
    let gateway_class_rx = gateway_class_tx.clone();
    let gateways_rx = gateways_rx.clone();

    task_builder
        .new_task(stringify!(filter_gateways))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(((gateway_class_ref, _), gateways)) =
                    await_ready!(gateway_class_rx, gateways_rx)
                {
                    debug!("Filtering Gateways for GatewayClass: {}", gateway_class_ref);
                    let gateways = gateways
                        .iter()
                        .filter(|(_, _, gateway)| {
                            let current_ref = ObjectRef::of_kind::<GatewayClass>()
                                .name(&gateway.spec.gateway_class_name)
                                .build();
                            &current_ref == gateway_class_ref
                        })
                        .collect();

                    tx.set(gateways).await;
                }

                continue_on!(gateway_class_rx.changed(), gateways_rx.changed());
            }
        });

    rx
}

#[allow(dead_code)] // Public API for future gateway parameter filtering
pub fn filter_gateway_parameters(
    task_builder: &TaskBuilder,
    gateways_rx: &Receiver<Objects<Gateway>>,
    gateway_parameters_rx: &Receiver<Objects<GatewayParameters>>,
) -> Receiver<HashMap<ObjectRef, Arc<GatewayParameters>>> {
    let (tx, rx) = signal("filtered_gateway_parameters");
    let gateways_rx = gateways_rx.clone();
    let gateway_parameters_rx = gateway_parameters_rx.clone();

    task_builder
        .new_task(stringify!(filter_gateway_parameters))
        .spawn(async move {
            loop {
                if let ReadyState::Ready((gateways, gateway_parameters)) =
                    await_ready!(gateways_rx, gateway_parameters_rx)
                {
                    let new_parameters = gateways
                        .iter()
                        .filter_map(|(gateway_ref, _, gateway)| {
                            if let Some(parameters_ref) = &gateway
                                .spec
                                .infrastructure
                                .as_ref()
                                .and_then(|i| i.parameters_ref.as_ref())
                            {
                                let parameters_ref = ObjectRef::builder()
                                    .group(Some(parameters_ref.group.clone()))
                                    .kind(parameters_ref.kind.clone())
                                    .namespace(gateway_ref.namespace().clone())
                                    .name(&parameters_ref.name)
                                    .build();

                                if let Some(parameters) =
                                    gateway_parameters.get_by_ref(&parameters_ref)
                                {
                                    debug!(
                                        "Found parameters for gateway {}: {:?}",
                                        gateway_ref, parameters
                                    );
                                    Some((gateway_ref.clone(), parameters.clone()))
                                } else {
                                    debug!("No parameters found for gateway {}", gateway_ref);
                                    None
                                }
                            } else {
                                debug!("Gateway {} has no infrastructure defined", gateway_ref);
                                None
                            }
                        })
                        .collect();

                    tx.set(new_parameters).await;
                }

                continue_on!(gateways_rx.changed(), gateway_parameters_rx.changed());
            }
        });

    rx
}
