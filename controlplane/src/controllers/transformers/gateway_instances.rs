use crate::objects::{ObjectRef, Objects};
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use getset::Getters;
use kubera_api::v1alpha1::{GatewayClassParameters, GatewayParameters};
use kubera_core::continue_on;
use kubera_core::sync::signal::{channel, Receiver};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinSet;

#[derive(Clone, Debug, Getters, PartialEq)]
pub struct GatewayInstanceConfiguration {
    #[getset(get = "pub")]
    gateway: Arc<Gateway>,

    #[getset(get = "pub")]
    gateway_class_parameters: Option<Arc<GatewayClassParameters>>,

    #[getset(get = "pub")]
    gateway_parameters: Option<Arc<GatewayParameters>>,
}

pub fn collect_gateway_instances(
    join_set: &mut JoinSet<()>,
    gateways: &Receiver<Objects<Gateway>>,
    gateway_class_parameters: &Receiver<Option<Arc<GatewayClassParameters>>>,
    gateway_parameters: &Receiver<HashMap<ObjectRef, Arc<GatewayParameters>>>,
) -> Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>> {
    let (tx, rx) = channel(HashMap::new());

    let mut gateways = gateways.clone();
    let mut gateway_class_parameters = gateway_class_parameters.clone();
    let mut gateway_parameters = gateway_parameters.clone();

    join_set.spawn(async move {
        loop {
            let current_gateways = gateways.current();
            let current_gateway_class_parameters = gateway_class_parameters.current();
            let current_gateway_parameters = gateway_parameters.current();

            let instances = current_gateways
                .iter()
                .map(|(gateway_ref, _, gateway)| {
                    let gateway_parameters = current_gateway_parameters.get(&gateway_ref).cloned();
                    (
                        gateway_ref,
                        GatewayInstanceConfiguration {
                            gateway,
                            gateway_class_parameters: current_gateway_class_parameters
                                .as_ref()
                                .clone(),
                            gateway_parameters,
                        },
                    )
                })
                .collect();

            tx.replace(instances);

            continue_on!(
                gateways.changed(),
                gateway_class_parameters.changed(),
                gateway_parameters.changed()
            );
        }
    });

    rx
}
