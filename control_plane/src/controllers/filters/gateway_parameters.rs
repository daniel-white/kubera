use crate::kubernetes::objects::{ObjectRef, Objects};
use gateway_api::apis::standard::gateways::Gateway;
use kubera_api::v1alpha1::GatewayParameters;
use kubera_core::continue_on;
use kubera_core::sync::signal::{signal, Receiver};
use kubera_core::task::Builder as TaskBuilder;
use kubera_macros::await_ready;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

// Note: GatewayClassParametersReferenceState is defined in gateway_classes.rs

pub fn filter_gateway_parameters(
    task_builder: &TaskBuilder,
    gateways_rx: &Receiver<Objects<Gateway>>,
    gateway_parameters_rx: &Receiver<Objects<GatewayParameters>>,
) -> Receiver<Objects<GatewayParameters>> {
    let (tx, rx) = signal();
    let gateways_rx = gateways_rx.clone();
    let gateway_parameters_rx = gateway_parameters_rx.clone();

    task_builder
        .new_task(stringify!(filter_gateway_parameters))
        .spawn(async move {
            loop {
                await_ready!(gateways_rx, gateway_parameters_rx)
                    .and_then(async |_gateways, gateway_parameters| {
                        debug!("Filtering GatewayParameters");

                        // Filter gateway parameters based on referenced gateways
                        let filtered_parameters: Objects<GatewayParameters> = gateway_parameters
                            .iter()
                            .filter(|(_, _, _)| {
                                // For now, include all gateway parameters
                                // This can be refined based on specific filtering logic
                                true
                            })
                            .collect();

                        tx.set(filtered_parameters).await;
                    })
                    .run()
                    .await;

                continue_on!(gateways_rx.changed(), gateway_parameters_rx.changed());
            }
        });

    rx
}

pub fn transform_gateway_parameters_to_map(
    task_builder: &TaskBuilder,
    gateway_parameters_rx: &Receiver<Objects<GatewayParameters>>,
) -> Receiver<HashMap<ObjectRef, Arc<GatewayParameters>>> {
    let (tx, rx) = signal();
    let gateway_parameters_rx = gateway_parameters_rx.clone();

    task_builder
        .new_task(stringify!(transform_gateway_parameters_to_map))
        .spawn(async move {
            loop {
                await_ready!(gateway_parameters_rx)
                    .and_then(async |gateway_parameters| {
                        debug!("Transforming GatewayParameters to HashMap");

                        let parameters_map: HashMap<ObjectRef, Arc<GatewayParameters>> =
                            gateway_parameters
                                .iter()
                                .map(|(object_ref, _, gateway_param)| (object_ref, gateway_param))
                                .collect();

                        tx.set(parameters_map).await;
                    })
                    .run()
                    .await;

                continue_on!(gateway_parameters_rx.changed());
            }
        });

    rx
}
