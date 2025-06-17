use crate::objects::{ObjectRef, ObjectState, Objects};
use derive_builder::Builder;
use gateway_api::apis::standard::httproutes::HTTPRoute;
use getset::Getters;
use k8s_openapi::api::core::v1::Service;
use kubera_core::select_continue;
use kubera_core::sync::signal::{Receiver, channel};
use std::collections::BTreeMap;
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
) -> Receiver<BTreeMap<ObjectRef, HttpRouteBackend>> {
    let (tx, rx) = channel(BTreeMap::new());

    let mut http_routes = http_routes.clone();

    join_set.spawn(async move {
        loop {
            let current_http_routes = http_routes.current();
            let mut http_route_backends = BTreeMap::new();

            for (http_route_ref, _, http_route) in current_http_routes.iter() {
                match http_route {
                    ObjectState::Active(http_route) => {
                        info!(
                            "Collecting backends for HTTPRoute: object.ref={}",
                            http_route_ref
                        );
                        for rules in http_route.spec.rules.iter() {
                            for rule in rules {
                                for backend_ref in rule.backend_refs.iter().flatten() {
                                    if let Some(kind) = backend_ref.kind.as_deref() {
                                        if kind == "Service" {
                                            let object_ref = ObjectRef::new_builder()
                                                .of_kind::<Service>()
                                                .namespace(backend_ref.namespace.clone())
                                                .name(&backend_ref.name)
                                                .build()
                                                .unwrap();

                                            let http_route_backend =
                                                HttpRouteBackend::new_builder()
                                                    .object_ref(object_ref.clone())
                                                    .port(backend_ref.port)
                                                    .weight(backend_ref.weight)
                                                    .build()
                                                    .unwrap();
                                            http_route_backends
                                                .insert(object_ref, http_route_backend);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        debug!("Skipping deleted object.ref={}", http_route_ref);
                    }
                }
            }

            tx.replace(http_route_backends);

            select_continue!(http_routes.changed());
        }
    });

    rx
}
