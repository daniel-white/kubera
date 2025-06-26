use crate::objects::{ObjectRef, Objects};
use gateway_api::apis::standard::gateways::Gateway;
use gateway_api::apis::standard::httproutes::HTTPRoute;
use itertools::*;
use kubera_core::continue_on;
use kubera_core::sync::signal::{channel, Receiver};
use tokio::task::JoinSet;
use tracing::{debug, info};

pub fn filter_http_routes(
    join_set: &mut JoinSet<()>,
    gateways: &Receiver<Objects<Gateway>>,
    http_routes: &Receiver<Objects<HTTPRoute>>,
) -> Receiver<Objects<HTTPRoute>> {
    let (tx, rx) = channel(Objects::default());

    let gateways = gateways.clone();
    let http_routes = http_routes.clone();

    join_set.spawn(async move {
        loop {
            let current_gateways = gateways.current();
            let current_http_routes = http_routes.current();
            let filtered = current_http_routes
                .iter()
                .filter(|(http_route_ref, _, http_route)| {
                    let result = http_route
                        .spec
                        .parent_refs
                        .iter()
                        .flat_map(|parent_ref| parent_ref.iter())
                        .map(|parent_ref| {
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
                                .unwrap()
                        })
                        .unique()
                        .any(|r| current_gateways.contains_by_ref(&r));

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

            tx.replace(filtered);

            continue_on!(gateways.changed(), http_routes.changed());
        }
    });

    rx
}
