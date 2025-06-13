use crate::controllers::resources::{ObjectRef, ObjectState, Objects};
use gateway_api::apis::standard::gateways::Gateway;
use gateway_api::apis::standard::httproutes::HTTPRoute;
use itertools::*;
use kubera_core::select_continue;
use kubera_core::sync::signal::{Receiver, channel};
use tokio::task::JoinSet;
use tracing::{debug, info};

pub fn filter_http_routes(
    join_set: &mut JoinSet<()>,
    gateways: &Receiver<Objects<Gateway>>,
    http_routes: &Receiver<Objects<HTTPRoute>>,
) -> Receiver<Objects<HTTPRoute>> {
    let (tx, rx) = channel(Objects::default());

    let mut gateways = gateways.clone();
    let mut http_routes = http_routes.clone();

    join_set.spawn(async move {
        loop {
            let current_gateways = gateways.current();
            let filtered = http_routes.current().filter(|http_route_ref, http_route| {
                if let ObjectState::Active(http_route) = http_route {
                    let result = http_route
                        .spec
                        .parent_refs
                        .iter()
                        .flat_map(|parent_ref| parent_ref.iter().map(|p| p))
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
                        .any(|r| current_gateways.is_active(&r));

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
                } else {
                    false
                }
            });

            tx.replace(filtered);

            select_continue!(gateways.changed(), http_routes.changed());
        }
    });

    rx
}
