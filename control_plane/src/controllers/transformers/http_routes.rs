use crate::kubernetes::objects::{ObjectRef, Objects};
use derive_builder::Builder;
use gateway_api::apis::standard::gateways::Gateway;
use gateway_api::apis::standard::httproutes::HTTPRoute;
use getset::{CopyGetters, Getters};
use k8s_openapi::api::core::v1::Service;
use kubera_core::continue_on;
use kubera_core::net::Port;
use kubera_core::sync::signal::{signal, Receiver};
use kubera_core::task::Builder as TaskBuilder;
use kubera_macros::await_ready;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

#[derive(Debug, Builder, Getters, CopyGetters, Clone, Hash, PartialEq, Eq)]
pub struct HttpRouteBackend {
    #[getset(get = "pub")]
    object_ref: ObjectRef,

    #[getset(get_copy = "pub")]
    port: Option<Port>,

    #[getset(get_copy = "pub")]
    weight: Option<i32>,
}

impl HttpRouteBackend {
    pub fn new_builder() -> HttpRouteBackendBuilder {
        HttpRouteBackendBuilder::default()
    }
}

pub fn collect_http_route_backends(
    task_builder: &TaskBuilder,
    http_routes_rx: &Receiver<Objects<HTTPRoute>>,
) -> Receiver<HashMap<ObjectRef, HttpRouteBackend>> {
    let (tx, rx) = signal();
    let http_routes_rx = http_routes_rx.clone();

    task_builder.new_task(stringify!(collect_http_route_backends)).spawn(async move {
        loop {
            await_ready!(http_routes_rx)
                .and_then(async |http_routes| {
                    let mut http_route_backends = HashMap::new();

                    for (http_route_ref, _, http_route) in http_routes.iter() {
                        info!(
                            "Collecting backends for HTTPRoute: object.ref={}",
                            http_route_ref
                        );
                        for (rule_idx, rule) in http_route.spec.rules.iter().flatten().enumerate() {
                            'backend_refs:
                            for (backend_idx, backend_ref) in rule.backend_refs.iter().flatten().enumerate() {
                                #[allow(clippy::single_match_else)] // We'll likely add more kinds later
                                match backend_ref.kind.as_deref() {
                                    Some("Service") => {
                                        let service_ref = match ObjectRef::new_builder()
                                            .of_kind::<Service>()
                                            .namespace(
                                                backend_ref
                                                    .namespace
                                                    .clone()
                                                    .or_else(|| http_route.metadata.namespace.clone()),
                                            )
                                            .name(&backend_ref.name)
                                            .build() {
                                            Ok(service_ref) => service_ref,
                                            Err(err) => {
                                                warn!(
                                                        "Failed to create service reference for HTTPRoute {http_route_ref} at rule index {rule_idx}, backend index {backend_idx}: {err}"
                                                    );
                                                continue 'backend_refs;
                                            }
                                        };

                                        let http_route_backend = match HttpRouteBackend::new_builder()
                                            .object_ref(service_ref.clone())
                                            .port(
                                                backend_ref
                                                    .port
                                                    .map(|p| {
                                                        u16::try_from(p).expect("Port must be u16")
                                                    })
                                                    .map(Port::new),
                                            )
                                            .weight(backend_ref.weight).build() {
                                            Ok(http_route_backend) => http_route_backend,
                                            Err(err) => {
                                                warn!(
                                                        "Failed to create HttpRouteBackend for HTTPRoute {http_route_ref} at rule index {rule_idx}, backend index {backend_idx}: {err}"
                                                    );
                                                continue 'backend_refs;
                                            }
                                        };

                                        http_route_backends.insert(service_ref, http_route_backend);
                                    }
                                    _ => {
                                        warn!(
                                                "Skipping backend reference at rule index {rule_idx}, backend index {backend_idx} for HTTPRoute {http_route_ref}: unsupported kind {}",
                                                backend_ref.kind.as_deref().unwrap_or("{unknown}")
                                            );
                                        continue 'backend_refs;
                                    }
                                }
                            }
                        }
                    }

                    tx.set(http_route_backends).await;
            }).run().await;

            continue_on!(http_routes_rx.changed());
        }
    });

    rx
}

pub fn collect_http_routes_by_gateway(
    task_builder: &TaskBuilder,
    http_routes_rx: &Receiver<Objects<HTTPRoute>>,
) -> Receiver<HashMap<ObjectRef, Vec<Arc<HTTPRoute>>>> {
    let (tx, rx) = signal();
    let http_routes_rx = http_routes_rx.clone();

    task_builder
        .new_task("collect_http_routes_by_gateway")
        .spawn(async move {
        loop {
            if let Some(http_routes) = http_routes_rx.get().await {
                info!("Collecting HTTPRoutes by Gateway");
                let mut new_routes: HashMap<ObjectRef, Vec<Arc<HTTPRoute>>> = HashMap::new();

                for (http_route_ref, _, http_route) in http_routes.iter() {
                    info!("Collecting HTTPRoute: object.ref={}", http_route_ref);
                    'parent_refs:
                    for (parent_idx, parent_ref) in http_route.spec.parent_refs.iter().flatten().enumerate() {
                        let gateway_ref = match ObjectRef::new_builder()
                            .of_kind::<Gateway>()
                            .namespace(
                                parent_ref
                                    .namespace
                                    .clone()
                                    .or_else(|| http_route_ref.namespace().clone()),
                            )
                            .name(&parent_ref.name)
                            .build()
                        {
                            Ok(gateway_ref) => gateway_ref,
                            Err(err) => {
                                warn!("Failed to create parent gateway reference for {http_route_ref} at index {parent_idx}: {err}");
                                continue 'parent_refs;
                            }
                        };

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

                tx.set(new_routes).await;
            }

            continue_on!(http_routes_rx.changed());
        }
    });

    rx
}
