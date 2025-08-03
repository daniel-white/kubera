use crate::kubernetes::objects::{ObjectRef, Objects};
use gateway_api::apis::standard::gateways::Gateway;
use gateway_api::apis::standard::httproutes::HTTPRoute;
use itertools::Itertools;
use kubera_core::continue_on;
use kubera_core::sync::signal::{Receiver, signal};
use kubera_core::task::Builder as TaskBuilder;
use kubera_macros::await_ready;
use tracing::{debug, debug_span, info};

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
                await_ready!(gateways_rx, http_routes_rx)
                    .and_then(async |gateways, http_routes| {
                        let http_routes = http_routes
                            .iter()
                            .filter(|(http_route_ref, _, http_route)| {
                                let result = debug_span!("inner").in_scope(|| { http_route
                                    .spec
                                    .parent_refs
                                    .iter()
                                    .flat_map(|parent_ref| parent_ref.iter())
                                    .map(|parent_ref| {
                                        ObjectRef::of_kind::<Gateway>() // Assuming v1 for simplicity, adjust as needed
                                            .namespace(
                                                parent_ref
                                                    .namespace
                                                    .clone()
                                                    .or_else(|| http_route_ref.namespace().clone()),
                                            )
                                            .name(&parent_ref.name)
                                            .build()
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

                        tx.set(http_routes).await;
                    })
                    .run()
                    .await;

                continue_on!(gateways_rx.changed(), http_routes_rx.changed());
            }
        });

    rx
}
