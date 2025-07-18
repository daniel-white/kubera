use crate::kubernetes::objects::{ObjectRef, Objects};
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use itertools::Itertools;
use kubera_api::constants::GATEWAY_CLASS_CONTROLLER_NAME;
use kubera_api::v1alpha1::GatewayClassParameters;
use kubera_core::continue_on;
use kubera_core::sync::signal::{signal, Receiver};
use kubera_core::task::Builder as TaskBuilder;
use kubera_macros::await_ready;
use std::sync::Arc;
use strum::{EnumString, IntoStaticStr};
use tracing::{debug, info, warn};

pub fn filter_gateway_classes(
    task_builder: &TaskBuilder,
    gateway_classes_rx: &Receiver<Objects<GatewayClass>>,
) -> Receiver<(ObjectRef, Arc<GatewayClass>)> {
    let (tx, rx) = signal();
    let gateway_classes_rx = gateway_classes_rx.clone();

    task_builder
        .new_task(stringify!(filter_gateway_classes))
        .spawn(async move {
            loop {
                await_ready!(gateway_classes_rx)
                    .and_then(async |gateway_classes| {
                        debug!("Filtering GatewayClasses");

                        let gateway_class = gateway_classes
                            .iter()
                            .filter_map(|(_, _, gateway_class)| {
                                if gateway_class.spec.controller_name
                                    == GATEWAY_CLASS_CONTROLLER_NAME
                                {
                                    Some(gateway_class)
                                } else {
                                    None
                                }
                            })
                            .exactly_one();

                        let gateway_class = match gateway_class {
                            Ok(gateway_class) => {
                                match ObjectRef::new_builder()
                                    .for_object(gateway_class.as_ref())
                                    .build()
                                {
                                    Ok(gateway_class_ref) => {
                                        info!(
                                            "Found GatewayClass: object.ref={}",
                                            gateway_class_ref
                                        );
                                        tx.set((gateway_class_ref, gateway_class)).await;
                                    }
                                    Err(e) => {
                                        warn!("Error filtering GatewayClass creating ref: {}", e);
                                    }
                                };
                            }
                            Err(e) => {
                                warn!("Error filtering GatewayClass: {}", e);
                            }
                        };
                    })
                    .run()
                    .await;

                continue_on!(gateway_classes_rx.changed());
            }
        });

    rx
}

#[derive(Debug, Clone, PartialEq, IntoStaticStr, EnumString)]
pub enum GatewayClassParametersReferenceState {
    #[strum(serialize = "kubera.whitefamily.in/NoRef")]
    NoRef,
    #[strum(serialize = "kubera.whitefamily.in/NoRef")]
    InvalidRef,
    #[strum(serialize = "kubera.whitefamily.in/NotFound")]
    NotFound,
    #[strum(serialize = "kubera.whitefamily.in/Linked")]
    Linked(Arc<GatewayClassParameters>),
}

impl Into<Option<Arc<GatewayClassParameters>>> for GatewayClassParametersReferenceState {
    fn into(self) -> Option<Arc<GatewayClassParameters>> {
        match self {
            Self::Linked(parameters) => Some(parameters),
            _ => None,
        }
    }
}

pub fn filter_gateway_class_parameters(
    task_builder: &TaskBuilder,
    gateway_class_rx: &Receiver<(ObjectRef, Arc<GatewayClass>)>,
    gateway_class_parameters_rx: &Receiver<Objects<GatewayClassParameters>>,
) -> Receiver<GatewayClassParametersReferenceState> {
    let (tx, rx) = signal();
    let gateway_class_rx = gateway_class_rx.clone();
    let gateway_class_parameters_rx = gateway_class_parameters_rx.clone();

    task_builder
        .new_task(stringify!(filter_gateway_class_parameters))
        .spawn(async move {
            loop {
                await_ready!(gateway_class_rx, gateway_class_parameters_rx)
                    .and_then(async |(gateway_class_ref, gateway_class), gateway_class_parameters| {
                        debug!("Filtering GatewayClassParameters for GatewayClass: {}", gateway_class_ref);

                        if let Some(parameters_ref) = &gateway_class.spec.parameters_ref {
                            match ObjectRef::new_builder()
                                .group(Some(parameters_ref.group.clone()))
                                .kind(&parameters_ref.kind)
                                .namespace(parameters_ref.namespace.clone())
                                .name(&parameters_ref.name)
                                .version("v1alpha1")
                                .build()
                            {
                                Ok(parameters_ref) => {
                                    match gateway_class_parameters.get_by_ref(&parameters_ref) {
                                        Some(parameters) => {
                                            info!("Found GatewayClassParameters: object.ref={}", parameters_ref);
                                            tx.set(GatewayClassParametersReferenceState::Linked(parameters)).await;
                                        }
                                        None => {
                                            warn!("GatewayClassParameters not found for reference: {}", parameters_ref);
                                            tx.set(GatewayClassParametersReferenceState::NotFound).await;
                                        }
                                    }
                                }
                                Err(err) => {
                                    warn!("Error creating GatewayClassParameters reference: {}", err);
                                    tx.set(GatewayClassParametersReferenceState::InvalidRef).await;
                                }
                            }
                        } else {
                            info!("GatewayClass {} does not have parameters_ref set", gateway_class_ref);
                            tx.set(GatewayClassParametersReferenceState::NoRef).await;
                        }
                    })
                    .run()
                    .await;

                continue_on!(
                    gateway_class_rx.changed(),
                    gateway_class_parameters_rx.changed()
                );
            }
        });

    rx
}
