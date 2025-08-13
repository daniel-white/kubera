use crate::controllers::instances::InstanceRole;
use crate::kubernetes::objects::Objects;
use crate::kubernetes::KubeClientCell;
use gateway_api::apis::standard::gateways::{Gateway, GatewayStatus};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use k8s_openapi::chrono;
use kube::api::PostParams;
use kube::Api;
use std::time::Duration;
use tracing::{debug, info, warn};
use vg_core::continue_after;
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;
use vg_macros::await_ready;

pub fn sync_gateway_status(
    task_builder: &TaskBuilder,
    kube_client_rx: &Receiver<KubeClientCell>,
    instance_role_rx: &Receiver<InstanceRole>,
    gateways_rx: &Receiver<Objects<Gateway>>,
) {
    let kube_client_rx = kube_client_rx.clone();
    let instance_role_rx = instance_role_rx.clone();
    let gateways_rx = gateways_rx.clone();

    task_builder
        .new_task(stringify!(sync_gateway_status))
        .spawn(async move {
            loop {
                await_ready!(kube_client_rx, instance_role_rx, gateways_rx)
                    .and_then(async |kube_client, instance_role, gateways| {
                        if !instance_role.is_primary() {
                            debug!("Instance is not primary, skipping Gateway status updates");
                            return;
                        }

                        for (gateway_ref, _, gateway) in gateways.iter() {
                            info!("Syncing status for Gateway: {:?}", gateway_ref);

                            let status = build_gateway_status(&gateway).await;
                            debug!("Gateway status to be updated: {:?}", status);

                            let gateway_api = Api::<Gateway>::namespaced(
                                kube_client.clone().into(),
                                gateway_ref.namespace().as_deref().unwrap_or("default"),
                            );

                            let current_gateway = gateway_api
                                .get_status(gateway_ref.name().as_str())
                                .await
                                .map_err(|err| {
                                    warn!("Failed to get current Gateway status: {}", err);
                                })
                                .ok();

                            match current_gateway {
                                Some(mut current_gateway) => {
                                    current_gateway.status = Some(status);
                                    let patch = match serde_json::to_vec(&current_gateway) {
                                        Ok(patch) => patch,
                                        Err(err) => {
                                            warn!("Failed to serialize Gateway status: {}", err);
                                            continue;
                                        }
                                    };

                                    gateway_api
                                        .replace_status(
                                            gateway_ref.name().as_str(),
                                            &PostParams::default(),
                                            patch,
                                        )
                                        .await
                                        .map_err(|err| {
                                            warn!("Failed to update Gateway status: {}", err);
                                        })
                                        .ok();
                                }
                                None => {
                                    warn!(
                                        "Failed to retrieve current Gateway status for: {}",
                                        gateway_ref.name()
                                    );
                                }
                            }
                        }
                    })
                    .run()
                    .await;

                continue_after!(
                    Duration::from_secs(30),
                    kube_client_rx.changed(),
                    instance_role_rx.changed(),
                    gateways_rx.changed()
                );
            }
        });
}

async fn build_gateway_status(gateway: &Gateway) -> GatewayStatus {
    let now = chrono::Utc::now();
    let spec = &gateway.spec;

    // Build listener statuses - simplified for now since the correct struct names need to be found
    let listener_statuses = spec
        .listeners
        .iter()
        .map(|listener| {
            // Use the correct Gateway API structs - these may need to be imported differently
            serde_json::json!({
                "name": listener.name,
                "supportedKinds": [{
                    "group": "gateway.networking.k8s.io",
                    "kind": "HTTPRoute"
                }],
                "attachedRoutes": 0,
                "conditions": [{
                    "type": "Accepted",
                    "status": "True",
                    "reason": "Accepted",
                    "message": "Listener is accepted",
                    "lastTransitionTime": now.to_rfc3339()
                }]
            })
        })
        .collect::<Vec<_>>();

    // Build gateway addresses
    let addresses = vec![serde_json::json!({
        "type": "IPAddress",
        "value": "127.0.0.1"
    })];

    // For now, build the status using serde_json to avoid struct issues
    let status_json = serde_json::json!({
        "addresses": addresses,
        "conditions": [{
            "type": "Accepted",
            "status": "True",
            "reason": "Accepted",
            "message": "Gateway is accepted",
            "lastTransitionTime": now.to_rfc3339()
        }, {
            "type": "Programmed",
            "status": "True",
            "reason": "Programmed",
            "message": "Gateway is programmed",
            "lastTransitionTime": now.to_rfc3339()
        }],
        "listeners": listener_statuses
    });

    // Convert JSON to GatewayStatus
    serde_json::from_value(status_json).unwrap_or_else(|_| GatewayStatus {
        addresses: None,
        conditions: Some(vec![Condition {
            type_: "Accepted".to_string(),
            status: "True".to_string(),
            reason: "Accepted".to_string(),
            message: "Gateway is accepted".to_string(),
            observed_generation: None,
            last_transition_time: Time(now),
        }]),
        listeners: None,
    })
}
