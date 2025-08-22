use crate::controllers::filters::GatewayClassParametersReferenceState;
use crate::controllers::instances::InstanceRole;
use crate::kubernetes::objects::ObjectRef;
use crate::kubernetes::KubeClientCell;
use gateway_api::constants::{GatewayClassConditionReason, GatewayClassConditionType};
use gateway_api::gatewayclasses::{GatewayClass, GatewayClassStatus};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use k8s_openapi::chrono;
use kube::api::PostParams;
use kube::Api;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_after, ReadyState};

pub fn sync_gateway_class_status(
    task_builder: &TaskBuilder,
    kube_client_rx: &Receiver<KubeClientCell>,
    instance_role_rx: &Receiver<InstanceRole>,
    gateway_class_rx: &Receiver<(ObjectRef, Arc<GatewayClass>)>,
    gateway_class_parameters_rx: &Receiver<GatewayClassParametersReferenceState>,
) {
    let kube_client_rx = kube_client_rx.clone();
    let instance_role_rx = instance_role_rx.clone();
    let gateway_class_rx = gateway_class_rx.clone();
    let gateway_class_parameters_rx = gateway_class_parameters_rx.clone();

    task_builder
        .new_task(stringify!(sync_gateway_class_status))
        .spawn(async move {
            loop {
                if let ReadyState::Ready((
                    kube_client,
                    instance_role,
                    (gateway_class_ref, _),
                    gateway_class_parameters,
                )) = await_ready!(
                    kube_client_rx,
                    instance_role_rx,
                    gateway_class_rx,
                    gateway_class_parameters_rx
                ) {
                    info!("Syncing status for GatewayClass: {:?}", gateway_class_ref);

                    let status = map_to_status(gateway_class_parameters);
                    debug!("GatewayClass status to be updated: {:?}", status);

                    let gateway_class_api = Api::<GatewayClass>::all(kube_client.deref().clone());
                    let current_gateway_class = gateway_class_api
                        .get_status(gateway_class_ref.name().as_str())
                        .await
                        .map_err(|err| {
                            warn!("Failed to get current GatewayClass status: {}", err);
                        })
                        .ok();

                    if let Some(mut current_gateway_class) = current_gateway_class {
                        if instance_role.is_primary() {
                            current_gateway_class.status = Some(status);
                            let patch = match serde_json::to_vec(&current_gateway_class) {
                                Ok(patch) => patch,
                                Err(err) => {
                                    warn!("Failed to serialize GatewayClass status: {}", err);
                                    continue;
                                }
                            };
                            let params = PostParams::default();
                            if let Err(err) = gateway_class_api
                                .replace_status(gateway_class_ref.name().as_str(), &params, patch)
                                .await
                            {
                                warn!("Failed to update GatewayClass status: {}", err);
                            }
                        } else {
                            debug!("Not updating GatewayClass status: not primary");
                        }
                    } else {
                        debug!("Not updating GatewayClass status: not found");
                    }
                }
                continue_after!(
                    Duration::from_secs(60),
                    kube_client_rx.changed(),
                    instance_role_rx.changed(),
                    gateway_class_rx.changed(),
                    gateway_class_parameters_rx.changed()
                );
            }
        });
}

fn map_to_status(parameters_state: &GatewayClassParametersReferenceState) -> GatewayClassStatus {
    use GatewayClassParametersReferenceState::{InvalidRef, Linked, NoRef, NotFound};
    let now = chrono::Utc::now();

    match parameters_state {
        Linked(_) | NoRef => GatewayClassStatus {
            conditions: Some(vec![Condition {
                type_: GatewayClassConditionType::Accepted.to_string(),
                status: "True".to_string(),
                reason: GatewayClassConditionReason::Accepted.to_string(),
                message: "Accepted".to_string(),
                observed_generation: None,
                last_transition_time: Time(now),
            }]),
        },
        InvalidRef => GatewayClassStatus {
            conditions: Some(vec![Condition {
                type_: "InvalidRef".to_string(),
                status: "False".to_string(),
                reason: GatewayClassConditionReason::InvalidParameters.to_string(),
                message: "parameters ref is invalid".to_string(),
                observed_generation: None,
                last_transition_time: Time(now),
            }]),
        },
        NotFound => GatewayClassStatus {
            conditions: Some(vec![Condition {
                type_: "NotFound".to_string(),
                status: "False".to_string(),
                reason: GatewayClassConditionReason::InvalidParameters.to_string(),
                message: "GatewayClassParameters not found".to_string(),
                observed_generation: None,
                last_transition_time: Time(now),
            }]),
        },
    }
}
