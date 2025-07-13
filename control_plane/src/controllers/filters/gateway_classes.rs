use crate::kubernetes::objects::{ObjectRef, Objects};
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use itertools::Itertools;
use kubera_api::constants::GATEWAY_CLASS_CONTROLLER_NAME;
use kubera_api::v1alpha1::GatewayClassParameters;
use kubera_core::continue_on;
use kubera_core::sync::signal::{signal, Receiver};
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{debug, info, warn};

pub fn filter_gateway_classes(
    join_set: &mut JoinSet<()>,
    gateway_classes_rx: &Receiver<Objects<GatewayClass>>,
) -> Receiver<(ObjectRef, GatewayClass)> {
    let (tx, rx) = signal();
    let gateway_classes_rx = gateway_classes_rx.clone();

    join_set.spawn(async move {
        loop {
            if let Some(gateway_classes) = gateway_classes_rx.get() {
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
                        tx.set((gateway_class_ref, gateway_class));
                    }
                    Err(e) => {
                        warn!("Error filtering GatewayClass creating ref: {}", e);
                    }
                };
            }

            continue_on!(gateway_classes_rx.changed());
        }
    });

    rx
}

pub fn filter_gateway_class_parameters(
    join_set: &mut JoinSet<()>,
    gateway_class_rx: &Receiver<(ObjectRef, GatewayClass)>,
    gateway_class_parameters_rx: &Receiver<Objects<GatewayClassParameters>>,
) -> Receiver<Arc<GatewayClassParameters>> {
    let (tx, rx) = signal();
    let gateway_class_rx = gateway_class_rx.clone();
    let gateway_class_parameters_rx = gateway_class_parameters_rx.clone();

    join_set.spawn(async move {
        loop {
            if let Some((_, gateway_class)) = &gateway_class_rx.get().as_deref()
                && let Some(parameters_ref) = &gateway_class.spec.parameters_ref
                && let Some(gateway_class_parameters) = gateway_class_parameters_rx.get()
            {
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
                        tx.replace(None);
                        continue_on!(
                            gateway_class_rx.changed(),
                            gateway_class_parameters_rx.changed()
                        );
                    }
                };
                let parameters = gateway_class_parameters.get_by_ref(&parameters_ref);

                tx.replace(parameters);
            }

            continue_on!(
                gateway_class_rx.changed(),
                gateway_class_parameters_rx.changed()
            );
        }
    });

    rx
}
