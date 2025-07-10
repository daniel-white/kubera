use crate::kubernetes::objects::{ObjectRef, Objects};
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use kubera_api::v1alpha1::GatewayParameters;
use kubera_core::continue_on;
use kubera_core::sync::signal::{Receiver, channel};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{debug, info, warn};

pub fn filter_gateways(
    join_set: &mut JoinSet<()>,
    gateway_class: &Receiver<Option<(ObjectRef, Arc<GatewayClass>)>>,
    gateways: &Receiver<Objects<Gateway>>,
) -> Receiver<Objects<Gateway>> {
    let (tx, rx) = channel(Objects::default());

    let gateway_class = gateway_class.clone();
    let gateways = gateways.clone();

    join_set.spawn(async move {
        loop {
            if let Some((expected_gateway_class_ref, _)) = gateway_class.current().as_ref() {
                let filtered = gateways
                    .current()
                    .iter()
                    .filter(|(gateway_ref, _, gateway)| {
                        ObjectRef::new_builder()
                            .of_kind::<GatewayClass>()
                            .namespace(None)
                            .name(&gateway.spec.gateway_class_name)
                            .build()
                            .inspect_err(|err| {
                                warn!(
                                    "Error creating GatewayClass reference for gateway {}: {}",
                                    gateway_ref, err
                                );
                            })
                            .ok()
                            .is_some_and(|gateway_class_ref| {
                                expected_gateway_class_ref == &gateway_class_ref
                            })
                    })
                    .collect();

                tx.replace(filtered);
            } else {
                info!("No GatewayClass supported available");
                tx.replace(Objects::default());
            }

            continue_on!(gateway_class.changed(), gateways.changed());
        }
    });

    rx
}

pub fn filter_gateway_parameters(
    join_set: &mut JoinSet<()>,
    gateways: &Receiver<Objects<Gateway>>,
    gateway_parameters: &Receiver<Objects<GatewayParameters>>,
) -> Receiver<HashMap<ObjectRef, Arc<GatewayParameters>>> {
    let (tx, rx) = channel(HashMap::default());

    let gateways = gateways.clone();
    let gateway_parameters = gateway_parameters.clone();

    join_set.spawn(async move {
        loop {
            let current_gateways = gateways.current();

            let new_parameters = current_gateways
                .iter()
                .filter_map(|(gateway_ref, _, gateway)| {
                    if let Some(parameters_ref) = &gateway
                        .spec
                        .infrastructure
                        .as_ref()
                        .and_then(|i| i.parameters_ref.as_ref())
                    {
                        let parameters_ref = ObjectRef::new_builder()
                            .group(Some(parameters_ref.group.clone()))
                            .kind(parameters_ref.kind.clone())
                            .namespace(gateway_ref.namespace().clone())
                            .name(&parameters_ref.name)
                            .build()
                            .expect("Failed to build parameters reference from Gateway");

                        let current_parameters = gateway_parameters.current();
                        if let Some(parameters) = current_parameters.get_by_ref(&parameters_ref) {
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

            tx.replace(new_parameters);

            continue_on!(gateways.changed(), gateway_parameters.changed());
        }
    });

    rx
}
