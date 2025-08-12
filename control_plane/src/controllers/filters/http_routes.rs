use crate::kubernetes::objects::{ObjectRef, Objects};
use gateway_api::apis::standard::gateways::Gateway;
use gateway_api::apis::standard::httproutes::HTTPRoute;
use tracing::{debug, debug_span, info, warn};
use vg_core::continue_on;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_macros::await_ready;

/// Check if an HTTP route is allowed by the Gateway's allowedRoutes configuration
fn is_http_route_allowed_by_gateway(
    http_route_ref: &ObjectRef,
    gateway: &Gateway,
    gateway_ref: &ObjectRef,
) -> bool {
    // Get the HTTP route's namespace
    let Some(http_route_namespace) = http_route_ref.namespace() else {
        warn!("HTTPRoute {} has no namespace, rejecting", http_route_ref);
        return false;
    };

    let Some(gateway_namespace) = gateway_ref.namespace() else {
        warn!(
            "Gateway {} has no namespace, rejecting HTTPRoute {}",
            gateway_ref, http_route_ref
        );
        return false;
    };

    // Check each listener in the Gateway
    for listener in &gateway.spec.listeners {
        // Check if this listener allows routes from the HTTP route's namespace
        if let Some(allowed_routes) = &listener.allowed_routes {
            if let Some(namespaces) = &allowed_routes.namespaces {
                match &namespaces.from {
                    Some(gateway_api::apis::standard::gateways::GatewayListenersAllowedRoutesNamespacesFrom::Same) | None => {
                        // Only allow routes from the same namespace as the Gateway
                        if http_route_namespace != gateway_namespace {
                            debug!(
                                "HTTPRoute {} from namespace {} rejected by Gateway {} listener {}: allowedRoutes.namespaces.from=Same",
                                http_route_ref,
                                http_route_namespace,
                                gateway_ref,
                                listener.name
                            );
                            continue;
                        }
                    }
                    Some(gateway_api::apis::standard::gateways::GatewayListenersAllowedRoutesNamespacesFrom::All) => {
                        // Allow routes from all namespaces
                        debug!(
                            "HTTPRoute {} from namespace {} allowed by Gateway {} listener {}: allowedRoutes.namespaces.from=All",
                            http_route_ref,
                            http_route_namespace,
                            gateway_ref,
                            listener.name
                        );
                    }
                    Some(gateway_api::apis::standard::gateways::GatewayListenersAllowedRoutesNamespacesFrom::Selector) => {
                        // TODO: Implement namespace selector logic
                        // For now, we'll need access to namespace objects to evaluate selectors
                        // This would require watching namespace objects and evaluating label selectors
                        warn!(
                            "Namespace selector filtering not yet implemented for Gateway {} listener {}, allowing HTTPRoute {} for now",
                            gateway_ref,
                            listener.name,
                            http_route_ref
                        );
                    }
                }
            } else {
                // Default behavior when namespaces is not specified is "Same"
                if http_route_namespace != gateway_namespace {
                    debug!(
                        "HTTPRoute {} from namespace {} rejected by Gateway {} listener {}: default allowedRoutes.namespaces.from=Same",
                        http_route_ref, http_route_namespace, gateway_ref, listener.name
                    );
                    continue;
                }
            }

            // If we reach here, the route is allowed by this listener
            return true;
        }

        // Default behavior when allowedRoutes is not specified is to allow routes from the same namespace
        if http_route_namespace == gateway_namespace {
            return true;
        }
    }

    // If no listener allows this route, reject it
    debug!(
        "HTTPRoute {} from namespace {} rejected by Gateway {}: no listeners allow routes from this namespace",
        http_route_ref, http_route_namespace, gateway_ref
    );
    false
}

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
                                let result = debug_span!("inner").in_scope(|| {
                                    // Check if the HTTP route references any existing gateway and is allowed by its allowedRoutes configuration
                                    http_route
                                        .spec
                                        .parent_refs
                                        .iter()
                                        .flat_map(|parent_ref| parent_ref.iter())
                                        .any(|parent_ref| {
                                            let gateway_ref = ObjectRef::of_kind::<Gateway>()
                                                .namespace(
                                                    parent_ref
                                                        .namespace
                                                        .clone()
                                                        .or_else(|| http_route_ref.namespace().clone()),
                                                )
                                                .name(&parent_ref.name)
                                                .build();

                                            // Check if the gateway exists
                                            if let Some(gateway) = gateways.get_by_ref(&gateway_ref) {
                                                // Check if this HTTP route is allowed by the gateway's allowedRoutes configuration
                                                is_http_route_allowed_by_gateway(http_route_ref, &gateway, &gateway_ref)
                                            } else {
                                                debug!(
                                                    "HTTPRoute {} references non-existent Gateway {}",
                                                    http_route_ref,
                                                    gateway_ref
                                                );
                                                false
                                            }
                                        })
                                });

                                if result {
                                    info!(
                                        "HTTPRoute object.ref={} matches an active Vale Gateway and is allowed by allowedRoutes configuration",
                                        http_route_ref
                                    );
                                } else {
                                    debug!(
                                        "HTTPRoute object.ref={} does not match any active Vale Gateway or is rejected by allowedRoutes configuration",
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
