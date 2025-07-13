use crate::kubernetes::objects::{ObjectRef, Objects};
use gateway_api::apis::standard::gateways::Gateway;
use gateway_api::apis::standard::httproutes::HTTPRoute;
use itertools::Itertools;
use kubera_core::continue_on;
use kubera_core::sync::signal::{Receiver, signal};
use kubera_core::task::Builder as TaskBuilder;
use tracing::{debug, debug_span, info, warn};

pub fn filter_http_routes(
    task_builder: &TaskBuilder,
    gateways_rx: &Receiver<Objects<Gateway>>,
    http_routes_rx: &Receiver<Objects<HTTPRoute>>,
) -> Receiver<Objects<HTTPRoute>> {
    let (tx, rx) = signal();
    let gateways_rx = gateways_rx.clone();
    let http_routes_rx = http_routes_rx.clone();

    task_builder
        .new_task(stringify!(filter_http_routes))
        .spawn(async move {
            loop {
                warn!("Waiting for HTTPRoute updates - continue please");
                if let Some(gateways) = gateways_rx.get()
                    && let Some(http_routes) = http_routes_rx.get()
                {
                    let http_routes = http_routes
                    .iter()
                    .filter(|(http_route_ref, _, http_route)| {
                        let result = debug_span!("inner").in_scope(|| { http_route
                            .spec
                            .parent_refs
                            .iter()
                            .flat_map(|parent_ref| parent_ref.iter())
                            .filter_map(|parent_ref| {
                                ObjectRef::new_builder()
                                    .of_kind::<Gateway>() // Assuming v1 for simplicity, adjust as needed
                                    .namespace(
                                        parent_ref
                                            .namespace
                                            .clone()
                                            .or_else(|| http_route_ref.namespace().clone()),
                                    )
                                    .name(parent_ref.name.clone())
                                    .build()
                                    .inspect_err(|err| {
                                        warn!(
                                            "Error creating Gateway reference for HTTPRoute {}: {}",
                                            http_route_ref, err
                                        );
                                    })
                                    .ok()
                            })
                            .unique()
                            .any(|r| gateways.contains_by_ref(&r)) });

                        if result {
                            info!(
                                "HTTPRoute object.ref={} matches an active Kubera Gateway",
                                http_route_ref
                            );
                        } else {
                            debug!(
                                "HTTPRoute object.ref={} does not match any active Kubera Gateway",
                                http_route_ref
                            );
                        }

                        result
                    })
                    .collect();

                    tx.set(http_routes);
                }
                warn!("Waiting for HTTPRoute updates");
                continue_on!(gateways_rx.changed(), http_routes_rx.changed());
            }
        });

    rx
}
