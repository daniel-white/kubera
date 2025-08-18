use crate::controllers::instances::InstanceRole;
use crate::kubernetes::KubeClientCell;
use crate::kubernetes::objects::{ObjectRef, Objects};
use gateway_api::apis::standard::httproutes::{HTTPRoute, HTTPRouteStatus};
use k8s_openapi::chrono;
use kube::Api;
use kube::api::PostParams;
use std::collections::HashMap;
use tracing::{debug, info, warn};
use vg_core::continue_after;
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;
use vg_macros::await_ready;

#[derive(Debug, Clone, PartialEq)]
pub enum RouteAttachmentState {
    Attached,
    NotAttached { reason: String },
    ConflictedHostname { reason: String },
    InvalidBackendRef { reason: String },
    NoMatchingListener { reason: String },
}

pub fn sync_http_route_status(
    task_builder: &TaskBuilder,
    kube_client_rx: &Receiver<KubeClientCell>,
    instance_role_rx: &Receiver<InstanceRole>,
    http_routes_rx: &Receiver<Objects<HTTPRoute>>,
    route_attachment_states: &Receiver<HashMap<ObjectRef, RouteAttachmentState>>,
) {
    let kube_client_rx = kube_client_rx.clone();
    let instance_role_rx = instance_role_rx.clone();
    let http_routes_rx = http_routes_rx.clone();
    let route_attachment_states = route_attachment_states.clone();

    task_builder
        .new_task(stringify!(sync_http_route_status))
        .spawn(async move {
            loop {
                await_ready!(
                    kube_client_rx,
                    instance_role_rx,
                    http_routes_rx,
                    route_attachment_states
                )
                .and_then(
                    async |kube_client, instance_role, http_routes, attachment_states| {
                        if !instance_role.is_primary() {
                            debug!("Instance is not primary, skipping HTTPRoute status updates");
                            return;
                        }

                        for (route_ref, _, route) in http_routes.iter() {
                            info!("Syncing status for HTTPRoute: {:?}", route_ref);

                            let attachment_state = attachment_states
                                .get(&route_ref)
                                .cloned()
                                .unwrap_or(RouteAttachmentState::NotAttached {
                                    reason: "Route not processed yet".to_string(),
                                });

                            let status = build_http_route_status(&route, attachment_state);
                            debug!("HTTPRoute status to be updated: {:?}", status);

                            let route_api = Api::<HTTPRoute>::namespaced(
                                (*kube_client).clone(),
                                route_ref.namespace().as_deref().unwrap_or("default"),
                            );

                            let current_route = route_api
                                .get_status(route_ref.name().as_str())
                                .await
                                .map_err(|err| {
                                    warn!("Failed to get current HTTPRoute status: {}", err);
                                })
                                .ok();

                            match current_route {
                                Some(mut current_route) => {
                                    current_route.status = Some(status);
                                    let patch = match serde_json::to_vec(&current_route) {
                                        Ok(patch) => patch,
                                        Err(err) => {
                                            warn!("Failed to serialize HTTPRoute status: {}", err);
                                            continue;
                                        }
                                    };

                                    route_api
                                        .replace_status(
                                            route_ref.name().as_str(),
                                            &PostParams::default(),
                                            patch,
                                        )
                                        .await
                                        .map_err(|err| {
                                            warn!("Failed to update HTTPRoute status: {}", err);
                                        })
                                        .ok();
                                }
                                None => {
                                    warn!(
                                        "Failed to retrieve current HTTPRoute status for: {}",
                                        route_ref
                                    );
                                }
                            }
                        }
                    },
                )
                .run()
                .await;

                continue_after!(
                    std::time::Duration::from_secs(30),
                    kube_client_rx.changed(),
                    instance_role_rx.changed(),
                    http_routes_rx.changed(),
                    route_attachment_states.changed()
                );
            }
        });
}

fn build_http_route_status(
    route: &HTTPRoute,
    attachment_state: RouteAttachmentState,
) -> HTTPRouteStatus {
    let now = chrono::Utc::now();

    // Build parent statuses using serde_json to avoid struct issues
    let parent_statuses = route
        .spec
        .parent_refs
        .as_ref()
        .map(|parent_refs| {
            parent_refs
                .iter()
                .map(|parent_ref| {
                    let (condition_status, reason, message) = match &attachment_state {
                        RouteAttachmentState::Attached => (
                            "True",
                            "Accepted",
                            "Route is accepted and attached to the gateway",
                        ),
                        RouteAttachmentState::NotAttached { reason } => (
                            "False",
                            "NotAllowedByListeners",
                            reason.as_str(),
                        ),
                        RouteAttachmentState::ConflictedHostname { reason } => (
                            "False",
                            "NoMatchingListenerHostname",
                            reason.as_str(),
                        ),
                        RouteAttachmentState::InvalidBackendRef { reason } => (
                            "False",
                            "BackendNotFound",
                            reason.as_str(),
                        ),
                        RouteAttachmentState::NoMatchingListener { reason } => (
                            "False",
                            "NoMatchingParent",
                            reason.as_str(),
                        ),
                    };

                    // Build status using serde_json for now
                    let parent_status_json = serde_json::json!({
                        "parentRef": parent_ref,
                        "controllerName": "vale-gateway/controller",
                        "conditions": [{
                            "type": "Accepted",
                            "status": condition_status,
                            "reason": reason,
                            "message": message,
                            "lastTransitionTime": now.to_rfc3339()
                        }, {
                            "type": "ResolvedRefs",
                            "status": if matches!(attachment_state, RouteAttachmentState::InvalidBackendRef { .. }) { "False" } else { "True" },
                            "reason": if matches!(attachment_state, RouteAttachmentState::InvalidBackendRef { .. }) { "BackendNotFound" } else { "ResolvedRefs" },
                            "message": if matches!(attachment_state, RouteAttachmentState::InvalidBackendRef { .. }) { "Backend references could not be resolved" } else { "All references are resolved" },
                            "lastTransitionTime": now.to_rfc3339()
                        }]
                    });

                    parent_status_json
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Build the status using serde_json
    let status_json = serde_json::json!({
        "parents": parent_statuses
    });

    // Convert to HTTPRouteStatus or return a minimal version
    serde_json::from_value(status_json).unwrap_or_else(|_| HTTPRouteStatus { parents: vec![] })
}
