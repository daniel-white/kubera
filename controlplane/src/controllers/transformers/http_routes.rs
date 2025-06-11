use crate::controllers::resources::{Ref, ResourceState, Resources, ServiceBackend};
use gateway_api::apis::standard::httproutes::HTTPRoute;
use kubera_core::select_continue;
use kubera_core::sync::signal::{channel, Receiver};
use std::collections::BTreeMap;
use tokio::task::JoinSet;

pub fn collect_http_route_service_backends(
    join_set: &mut JoinSet<()>,
    http_routes: &Receiver<Resources<HTTPRoute>>,
) -> Receiver<BTreeMap<Ref, ServiceBackend>> {
    let (tx, rx) = channel(BTreeMap::new());

    let mut http_routes = http_routes.clone();

    join_set.spawn(async move {
        loop {
            let mut backends = BTreeMap::new();
            for (_, http_route) in http_routes.current().iter() {
                match http_route {
                    ResourceState::Active(http_route) => {
                        for rules in http_route.spec.rules.iter() {
                            for rule in rules {
                                for backend in rule.backend_refs.iter().flatten() {
                                    if let Some(kind) = backend.kind.as_deref() {
                                        if kind == "Service" {
                                            let ref_ = Ref::new_builder()
                                                .namespace(backend.namespace.clone())
                                                .name(&backend.name)
                                                .build()
                                                .unwrap();
                                            let backend = ServiceBackend::new_builder()
                                                .port(backend.port)
                                                .weight(backend.weight)
                                                .build()
                                                .unwrap();
                                            backends.insert(ref_, backend);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => continue,
                }
            }

            tx.replace(backends);

            select_continue!(http_routes.changed());
        }
    });

    rx
}
