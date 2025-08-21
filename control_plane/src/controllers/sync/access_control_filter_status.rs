use crate::kubernetes::KubeClientCell;
use crate::kubernetes::objects::Objects;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use k8s_openapi::chrono::Utc;
use kube::api::{Patch, PatchParams};
use kube::{Api, Client};
use tracing::{info, warn};
use vg_api::v1alpha1::{AccessControlFilter, AccessControlFilterStatus};
use vg_core::continue_on;
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;
use vg_macros::await_ready;

async fn patch_access_control_filter_status(
    client: Client,
    namespace: &str,
    name: &str,
    status: &impl serde::Serialize,
) -> Result<(), kube::Error> {
    let api: Api<AccessControlFilter> = Api::namespaced(client, namespace);
    let patch = serde_json::json!({ "status": status });
    api.patch_status(
        name,
        &PatchParams::apply("vg-control-plane"),
        &Patch::Merge(&patch),
    )
    .await?;
    Ok(())
}

pub fn sync_access_control_filter_status(
    task_builder: &TaskBuilder,
    access_control_filters_rx: &Receiver<Objects<AccessControlFilter>>,
    kube_client_rx: &Receiver<KubeClientCell>,
) {
    let kube_client_rx = kube_client_rx.clone();
    let access_control_filters_rx = access_control_filters_rx.clone();

    task_builder
        .new_task(stringify!(sync_access_control_filter_status))
        .spawn(async move {
            loop {
                await_ready!(access_control_filters_rx, kube_client_rx)
                    .and_then(async |access_control_filters, kube_client| {
                        for (ref_, _, _) in access_control_filters.iter() {
                            let now = Utc::now();
                            let status = AccessControlFilterStatus {
                                conditions: Some(vec![Condition {
                                    type_: "Ready".to_string(),
                                    status: "True".to_string(),
                                    reason: "Reconciled".to_string(),
                                    message: "AccessControlFilter is active and attached"
                                        .to_string(),
                                    last_transition_time: Time(now),
                                    observed_generation: None,
                                }]),
                                attached_routes: 0, // TODO: Count attached routes
                                last_updated: None,
                            };
                            info!("Updating status for AccessControlFilter: {}", ref_);
                            if let (Some(ns), name) = (ref_.namespace(), ref_.name()) {
                                if let Err(e) = patch_access_control_filter_status(
                                    kube_client.clone().into(),
                                    ns,
                                    name,
                                    &status,
                                )
                                .await
                                {
                                    warn!("Failed to patch status for {}: {}", ref_, e);
                                }
                            }
                        }
                    })
                    .run()
                    .await;
                continue_on!(
                    access_control_filters_rx.changed(),
                    kube_client_rx.changed()
                );
            }
        });
}
