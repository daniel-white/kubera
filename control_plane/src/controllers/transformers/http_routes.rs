use crate::controllers::sync::RouteAttachmentState;
use crate::kubernetes::objects::{ObjectRef, Objects};
use gateway_api::apis::standard::gateways::Gateway;
use gateway_api::apis::standard::httproutes::HTTPRoute;
use getset::{CopyGetters, Getters};
use k8s_openapi::api::core::v1::Service;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use tracing::{debug, info, warn};
use typed_builder::TypedBuilder;
use vg_core::continue_on;
use vg_core::net::Port;
use vg_core::sync::signal::{Receiver, signal};
use vg_core::task::Builder as TaskBuilder;
use vg_macros::await_ready;

#[derive(Debug, TypedBuilder, Getters, CopyGetters, Clone, Hash, PartialEq, Eq)]
pub struct HttpRouteBackend {
    #[getset(get = "pub")]
    object_ref: ObjectRef,

    #[getset(get_copy = "pub")]
    port: Option<Port>,

    #[getset(get_copy = "pub")]
    weight: Option<i32>,
}

pub fn collect_http_route_backends(
    task_builder: &TaskBuilder,
    http_routes_rx: &Receiver<Objects<HTTPRoute>>,
) -> Receiver<HashMap<ObjectRef, HttpRouteBackend>> {
    let (tx, rx) = signal("collected_http_route_backends");
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
                                #[allow(
                                    clippy::single_match_else
                                )] // We'll likely add more kinds later
                                match backend_ref.kind.as_deref() {
                                    Some("Service") => {
                                        let service_ref = ObjectRef::of_kind::<Service>()
                                            .namespace(
                                                backend_ref
                                                    .namespace
                                                    .clone()
                                                    .or_else(|| http_route.metadata.namespace.clone()),
                                            )
                                            .name(&backend_ref.name)
                                            .build();

                                        let http_route_backend = HttpRouteBackend::builder()
                                            .object_ref(service_ref.clone())
                                            .port(
                                                backend_ref
                                                    .port
                                                    .and_then(|port| {
                                                        u16::try_from(port)
                                                            .inspect_err(|err| {
                                                                warn!(
                                                                    "Invalid port {port} for backend reference at rule index {rule_idx}, backend index {backend_idx} for HTTPRoute {http_route_ref}: {err}"
                                                                );
                                                            })
                                                            .ok()
                                                    })
                                                    .map(Port::new),
                                            )
                                            .weight(backend_ref.weight).build();

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
    let (tx, rx) = signal("collected_http_routes_by_gateway");
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

                        for parent_ref in http_route.spec.parent_refs.iter().flatten() {
                            let gateway_ref = ObjectRef::of_kind::<Gateway>()
                                .namespace(
                                    parent_ref
                                        .namespace
                                        .clone()
                                        .or_else(|| http_route_ref.namespace().clone()),
                                )
                                .name(&parent_ref.name)
                                .build();

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

pub fn determine_route_attachment_states(
    task_builder: &TaskBuilder,
    http_routes_rx: &Receiver<Objects<HTTPRoute>>,
    gateways_rx: &Receiver<Objects<Gateway>>,
) -> Receiver<HashMap<ObjectRef, RouteAttachmentState>> {
    let (tx, rx) = signal("determined_route_attachment_states");
    let http_routes_rx = http_routes_rx.clone();
    let gateways_rx = gateways_rx.clone();

    task_builder
        .new_task("determine_route_attachment_states")
        .spawn(async move {
            loop {
                await_ready!(http_routes_rx, gateways_rx)
                    .and_then(async |http_routes, gateways| {
                        info!("Determining Route Attachment States");
                        let mut states: HashMap<ObjectRef, RouteAttachmentState> = HashMap::new();

                        for (http_route_ref, _, http_route) in http_routes.iter() {
                            info!(
                                "Determining state for HTTPRoute: object.ref={}",
                                http_route_ref
                            );

                            // Default to attached - we'll implement more sophisticated logic later
                            let state = if http_route.spec.parent_refs.is_some() {
                                // Check if any parent gateways exist
                                let mut has_valid_parent = false;
                                for parent_ref in http_route.spec.parent_refs.iter().flatten() {
                                    let gateway_ref = ObjectRef::of_kind::<Gateway>()
                                        .namespace(
                                            parent_ref
                                                .namespace
                                                .clone()
                                                .or_else(|| http_route_ref.namespace().clone()),
                                        )
                                        .name(&parent_ref.name)
                                        .build();

                                    if gateways.iter().any(|(gw_ref, _, _)| gw_ref == gateway_ref) {
                                        has_valid_parent = true;
                                        break;
                                    }
                                }

                                if has_valid_parent {
                                    RouteAttachmentState::Attached
                                } else {
                                    RouteAttachmentState::NoMatchingListener {
                                        reason: "No matching parent Gateway found".to_string(),
                                    }
                                }
                            } else {
                                RouteAttachmentState::NotAttached {
                                    reason: "No parent references specified".to_string(),
                                }
                            };

                            states.insert(http_route_ref.clone(), state);
                        }

                        tx.set(states).await;
                    })
                    .run()
                    .await;

                continue_on!(http_routes_rx.changed(), gateways_rx.changed());
            }
        });

    rx
}
