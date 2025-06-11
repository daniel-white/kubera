use crate::controllers::resources::{Ref, ResourceState, Resources};
use gateway_api::apis::standard::gateways::Gateway;
use gateway_api::apis::standard::httproutes::HTTPRoute;
use itertools::*;
use kubera_core::select_continue;
use kubera_core::sync::signal::{Receiver, channel};
use tokio::task::JoinSet;

pub fn filter_http_routes(
    join_set: &mut JoinSet<()>,
    gateways: &Receiver<Resources<Gateway>>,
    http_routes: &Receiver<Resources<HTTPRoute>>,
) -> Receiver<Resources<HTTPRoute>> {
    let (tx, rx) = channel(Resources::default());

    let mut gateways = gateways.clone();
    let mut http_routes = http_routes.clone();

    join_set.spawn(async move {
        loop {
            let current_gateways = gateways.current();
            let filtered = http_routes.current().filter_into(|_, http_route| {
                if let ResourceState::Active(http_route) = http_route {
                    http_route
                        .spec
                        .parent_refs
                        .iter()
                        .flat_map(|p| p.iter().map(|p| p))
                        .map(|p| {
                            Ref::new_builder()
                                .namespace(p.namespace.clone())
                                .name(&p.name)
                                .build()
                                .unwrap()
                        })
                        .unique()
                        .any(|r| current_gateways.is_active(&r))
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
