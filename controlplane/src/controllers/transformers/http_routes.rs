use crate::objects::{ObjectRef, Objects};
use derive_builder::Builder;
use gateway_api::apis::standard::gateways::Gateway;
use gateway_api::apis::standard::httproutes::HTTPRoute;
use getset::Getters;
use k8s_openapi::api::core::v1::Service;
use kubera_core::continue_on;
use kubera_core::sync::signal::{Receiver, channel};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{debug, info};

#[derive(Debug, Builder, Getters, Clone, Hash, PartialEq, Eq)]
pub struct HttpRouteBackend {
    #[getset(get = "pub")]
    object_ref: ObjectRef,

    #[getset(get = "pub")]
    port: Option<i32>,

    #[getset(get = "pub")]
    weight: Option<i32>,
}

impl HttpRouteBackend {
    pub fn new_builder() -> HttpRouteBackendBuilder {
        HttpRouteBackendBuilder::default()
    }
}

pub fn collect_http_route_backends(
    join_set: &mut JoinSet<()>,
    http_routes: &Receiver<Objects<HTTPRoute>>,
) -> Receiver<HashMap<ObjectRef, HttpRouteBackend>> {
    let (tx, rx) = channel(HashMap::new());

    let mut http_routes = http_routes.clone();

    join_set.spawn(async move {
        loop {
            let current_http_routes = http_routes.current();
            let mut http_route_backends = HashMap::new();

            for (http_route_ref, _, http_route) in current_http_routes.iter() {
                info!(
                    "Collecting backends for HTTPRoute: object.ref={}",
                    http_route_ref
                );
                for rules in http_route.spec.rules.iter() {
                    for rule in rules {
                        for backend_ref in rule.backend_refs.iter().flatten() {
                            if let Some(kind) = backend_ref.kind.as_deref() {
                                if kind == "Service" {
                                    let service_ref = ObjectRef::new_builder()
                                        .of_kind::<Service>()
                                        .namespace(
                                            backend_ref
                                                .namespace
                                                .clone()
                                                .or_else(|| http_route.metadata.namespace.clone()),
                                        )
                                        .name(&backend_ref.name)
                                        .build()
                                        .unwrap();

                                    let http_route_backend = HttpRouteBackend::new_builder()
                                        .object_ref(service_ref.clone())
                                        .port(backend_ref.port)
                                        .weight(backend_ref.weight)
                                        .build()
                                        .unwrap();
                                    http_route_backends.insert(service_ref, http_route_backend);
                                }
                            }
                        }
                    }
                }
            }

            tx.replace(http_route_backends);

            continue_on!(http_routes.changed());
        }
    });

    rx
}

pub fn collect_http_routes_by_gateway(
    join_set: &mut JoinSet<()>,
    gateways: &Receiver<Objects<Gateway>>,
    http_routes: &Receiver<Objects<HTTPRoute>>,
) -> Receiver<HashMap<ObjectRef, Vec<Arc<HTTPRoute>>>> {
    let (tx, rx) = channel(HashMap::new());

    let mut gateways = gateways.clone();
    let mut http_routes = http_routes.clone();

    join_set.spawn(async move {
        loop {
            let mut new_routes: HashMap<ObjectRef, Vec<Arc<HTTPRoute>>> = HashMap::new();

            for (http_route_ref, _, http_route) in http_routes.current().iter() {
                info!("Collecting HTTPRoute: object.ref={}", http_route_ref);
                for parent_ref in http_route.spec.parent_refs.iter().flatten() {
                    let gateway_ref = ObjectRef::new_builder()
                        .of_kind::<Gateway>()
                        .namespace(
                            parent_ref
                                .namespace
                                .clone()
                                .or_else(|| http_route_ref.namespace().clone()),
                        )
                        .name(&parent_ref.name)
                        .build()
                        .unwrap();

                    match new_routes.entry(gateway_ref) {
                        Entry::Occupied(mut entry) => {
                            entry.get_mut().push(http_route.clone());
                        }
                        Entry::Vacant(entry) => {
                            entry.insert(vec![http_route.clone()]);
                        }
                    }
                }
            }

            tx.replace(new_routes);

            continue_on!(gateways.changed(), http_routes.changed());
        }
    });

    rx
}
