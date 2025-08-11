use crate::kubernetes::objects::Objects;
use gateway_api::apis::standard::gateways::Gateway;
use vg_api::v1alpha1::GatewayParameters;
use vg_core::continue_on;
use vg_core::sync::signal::{Receiver, signal};
use vg_core::task::Builder as TaskBuilder;
use vg_macros::await_ready;
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
