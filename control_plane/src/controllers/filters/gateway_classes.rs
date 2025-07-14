use crate::kubernetes::objects::{ObjectRef, Objects};
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use itertools::Itertools;
use kubera_api::constants::GATEWAY_CLASS_CONTROLLER_NAME;
use kubera_api::v1alpha1::GatewayClassParameters;
use kubera_core::continue_on;
use kubera_core::sync::signal::{Receiver, signal};
use kubera_core::task::Builder as TaskBuilder;
use std::sync::Arc;
use tracing::{debug, info, warn};

pub fn filter_gateway_classes(
    task_builder: &TaskBuilder,
    gateway_classes_rx: &Receiver<Objects<GatewayClass>>,
) -> Receiver<(ObjectRef, GatewayClass)> {
    let (tx, rx) = signal();
    let gateway_classes_rx = gateway_classes_rx.clone();

    task_builder
        .new_task(stringify!(filter_gateway_classes))
        .spawn(async move {
            loop {
                if let Some(gateway_classes) = gateway_classes_rx.get().await {
                    debug!("Filtering GatewayClasses");

                    let gateway_class = gateway_classes
                        .iter()
                        .filter_map(|(_, _, gateway_class)| {
                            if gateway_class.spec.controller_name == GATEWAY_CLASS_CONTROLLER_NAME {
                                Some(gateway_class)
                            } else {
                                None
                            }
                        })
                        .exactly_one();

                    let gateway_class = match gateway_class {
                        Ok(gateway_class) => gateway_class.as_ref().clone(),
                        Err(e) => {
                            warn!("Error filtering GatewayClass: {}", e);
                            continue_on!(gateway_classes_rx.changed());
                        }
                    };

                    match ObjectRef::new_builder().for_object(&gateway_class).build() {
                        Ok(gateway_class_ref) => {
                            info!("Found GatewayClass: object.ref={}", gateway_class_ref);
                            tx.set((gateway_class_ref, gateway_class)).await;
                        }
                        Err(e) => {
                            warn!("Error filtering GatewayClass creating ref: {}", e);
                        }
                    };
                }

                debug!("Waiting for GatewayClass updates");
                continue_on!(gateway_classes_rx.changed());
            }
        });

    rx
}

pub fn filter_gateway_class_parameters(
    task_builder: &TaskBuilder,
    gateway_class_rx: &Receiver<(ObjectRef, GatewayClass)>,
    gateway_class_parameters_rx: &Receiver<Objects<GatewayClassParameters>>,
) -> Receiver<Option<Arc<GatewayClassParameters>>> {
    let (tx, rx) = signal();
    let gateway_class_rx = gateway_class_rx.clone();
    let gateway_class_parameters_rx = gateway_class_parameters_rx.clone();

    task_builder
        .new_task(stringify!(filter_gateway_class_parameters))
        .spawn(async move {
            loop {
                match  ( gateway_class_rx.get().await, gateway_class_parameters_rx.get().await) {
                    (Some((gateway_class_ref, gateway_class)), Some(gateway_class_parameters)) => {
                        info!("Filtering GatewayClassParameters for GatewayClass: {}", gateway_class_ref);

                        if  let Some(parameters_ref) = &gateway_class.spec.parameters_ref {
                            let parameters_ref = match ObjectRef::new_builder()
                                .group(Some(parameters_ref.group.clone()))
                                .kind(&parameters_ref.kind)
                                .namespace(parameters_ref.namespace.clone())
                                .name(&parameters_ref.name)
                                .version("v1alpha1")
                                .build()
                            {
                                Ok(parameters_ref) => parameters_ref,
                                Err(e) => {
                                    warn!("Error creating GatewayClassParameters reference: {}", e);
                                    tx.clear().await;
                                    continue_on!(
                                    gateway_class_rx.changed(),
                                    gateway_class_parameters_rx.changed()
                                );
                                }
                            };
                            let parameters = gateway_class_parameters.get_by_ref(&parameters_ref);

                            tx.set(parameters).await;
                        } else {
                            info!("GatewayClass {} does not have parameters_ref set", gateway_class_ref);
                            tx.set(None).await;
                        }
                    }
                    (None, _) => {
                        info!("No GatewayClass found, skipping GatewayClassParameters filtering");
                    }
                    (_, None) => {
                        info!("No GatewayClassParameters found, skipping GatewayClassParameters filtering");
                    }
                }

                continue_on!(
                    gateway_class_rx.changed(),
                    gateway_class_parameters_rx.changed()
                );
            }
        });

    rx
}
