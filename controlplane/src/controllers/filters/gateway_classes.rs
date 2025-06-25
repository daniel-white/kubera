use crate::objects::{ObjectRef, Objects};
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use itertools::Itertools;
use kubera_api::constants::GATEWAY_CLASS_CONTROLLER_NAME;
use kubera_api::v1alpha1::GatewayClassParameters;
use kubera_core::continue_on;
use kubera_core::sync::signal::{channel, Receiver};
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{debug, info, warn};

pub fn filter_gateway_classes(
    join_set: &mut JoinSet<()>,
    gateway_classes: &Receiver<Objects<GatewayClass>>,
) -> Receiver<Option<(ObjectRef, Arc<GatewayClass>)>> {
    let (tx, rx) = channel(None);

    let mut gateway_classes = gateway_classes.clone();

    join_set.spawn(async move {
        loop {
            let current_gateway_classes = gateway_classes.current();

            if !current_gateway_classes.is_empty() {
                let gateway_class = current_gateway_classes
                    .iter()
                    .filter(|(_, _, gateway_class)| {
                        gateway_class.spec.controller_name == GATEWAY_CLASS_CONTROLLER_NAME
                    })
                    .map(|(_, _, gateway_class)| gateway_class)
                    .exactly_one();

                match gateway_class {
                    Ok(gateway_class) => {
                        let gateway_class_ref = ObjectRef::new_builder()
                            .from_object(gateway_class.as_ref())
                            .build()
                            .unwrap();

                        info!("Found GatewayClass: object.ref={}", gateway_class_ref);
                        tx.replace(Some((gateway_class_ref, gateway_class)));
                    }
                    Err(e) => {
                        warn!("Error filtering GatewayClass: {}", e);
                        tx.replace(None);
                    }
                }
            } else {
                info!("No GatewayClass supported available");
                tx.replace(None);
            }

            continue_on!(gateway_classes.changed());
        }
    });

    rx
}

pub fn filter_gateway_class_parameters(
    join_set: &mut JoinSet<()>,
    gateway_class: &Receiver<Option<(ObjectRef, Arc<GatewayClass>)>>,
    gateway_class_parameters: &Receiver<Objects<GatewayClassParameters>>,
) -> Receiver<Option<Arc<GatewayClassParameters>>> {
    let (tx, rx) = channel(None);

    let mut gateway_class = gateway_class.clone();
    let mut gateway_class_parameters = gateway_class_parameters.clone();

    join_set.spawn(async move {
        loop {
            if let Some((_, gateway_class)) = gateway_class.current().as_ref() {
                match &gateway_class.spec.parameters_ref {
                    Some(parameters_ref) => {
                        let parameters_ref = ObjectRef::new_builder()
                            .group(Some(parameters_ref.group.clone()))
                            .kind(parameters_ref.kind.clone())
                            .namespace(parameters_ref.namespace.clone())
                            .name(&parameters_ref.name)
                            .version("v1alpha1")
                            .build()
                            .expect("Failed to build parameters reference from GatewayClass");

                        let current_parameters = gateway_class_parameters.current();
                        let parameters = current_parameters.get_by_ref(&parameters_ref);

                        tx.replace(parameters);
                    }
                    None => {
                        debug!("GatewayClass has no parameters");
                        tx.replace(None);
                    }
                }
            } else {
                info!("No GatewayClass supported available");
                tx.replace(None);
            }

            continue_on!(gateway_class.changed(), gateway_class_parameters.changed());
        }
    });

    rx
}
