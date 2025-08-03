use crate::controllers::instances::InstanceRole;
use crate::controllers::transformers::GatewayInstanceConfiguration;
use crate::kubernetes::KubeClientCell;
use crate::kubernetes::objects::{ObjectRef, SyncObjectAction};
use crate::options::Options;
use crate::{sync_objects, watch_objects};
use gtmpl_derive::Gtmpl;
use k8s_openapi::api::apps::v1::Deployment;
use kube::runtime::watcher::Config;
use kubera_core::continue_after;
use kubera_core::sync::signal::Receiver;
use kubera_core::task::Builder as TaskBuilder;
use kubera_macros::await_ready;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tracing::warn;
use typed_builder::TypedBuilder;

const TEMPLATE: &str = include_str!("./templates/gateway_deployment.kubernetes-helm-yaml");

#[derive(Clone, TypedBuilder, Debug, Gtmpl)]
struct TemplateValues {
    #[builder(setter(into))]
    gateway_name: String,
    #[builder(setter(into))]
    configmap_name: String,
    #[builder(setter(into))]
    image_pull_policy: String,
}

pub fn sync_gateway_deployments(
    options: Arc<Options>,
    task_builder: &TaskBuilder,
    kube_client_rx: &Receiver<KubeClientCell>,
    instance_role_rx: &Receiver<InstanceRole>,
    gateway_instances_rx: &Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
) {
    let (tx, current_service_refs_rx) = sync_objects!(
        options,
        task_builder,
        Deployment,
        kube_client_rx,
        instance_role_rx,
        TemplateValues,
        TEMPLATE
    );
    generate_gateway_deployments(
        options,
        task_builder,
        tx,
        current_service_refs_rx,
        gateway_instances_rx,
    );
}

fn generate_gateway_deployments(
    options: Arc<Options>,
    task_builder: &TaskBuilder,
    tx: Sender<SyncObjectAction<TemplateValues, Deployment>>,
    current_service_refs_rx: Receiver<HashSet<ObjectRef>>,
    gateway_instances_rx: &Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
) {
    let gateway_instances_rx = gateway_instances_rx.clone();

    task_builder
        .new_task(stringify!(generate_gateway_deployments))
        .spawn(async move {
            loop {
                await_ready!(gateway_instances_rx, current_service_refs_rx)
                    .and_then(async |gateway_instances, current_service_refs| {
                        let desired_deployments: Vec<_> = gateway_instances
                            .iter()
                            .map(|(gateway_ref, instance)| {
                                let deployment_ref = ObjectRef::of_kind::<Deployment>()
                                    .namespace(gateway_ref.namespace().clone())
                                    .name(gateway_ref.name())
                                    .build();

                                let template_values = TemplateValues::builder()
                                    .gateway_name(gateway_ref.name())
                                    .configmap_name(format!("{}-config", gateway_ref.name()))
                                    .image_pull_policy(Into::<&'static str>::into(
                                        instance.image_pull_policy(),
                                    ))
                                    .build();

                                (
                                    deployment_ref,
                                    gateway_ref,
                                    template_values,
                                    instance.deployment_overrides(),
                                )
                            })
                            .collect();

                        let desired_deployments_ref: HashSet<_> = desired_deployments
                            .iter()
                            .map(|(ref_, _, _, _)| ref_.clone())
                            .collect();

                        let deleted_refs =
                            current_service_refs.difference(&desired_deployments_ref);
                        for deleted_ref in deleted_refs {
                            tx.send(SyncObjectAction::Delete(deleted_ref.clone()))
                                .inspect_err(|err| {
                                    warn!(
                                        "Failed to send delete action for deployment {}: {}",
                                        deleted_ref, err
                                    );
                                })
                                .ok();
                        }

                        for (deployment_ref, gateway_ref, template_values, deployment_overrides) in
                            desired_deployments
                        {
                            tx.send(SyncObjectAction::Upsert(
                                deployment_ref.clone(),
                                gateway_ref.clone(),
                                template_values,
                                Some(deployment_overrides.clone()),
                            ))
                            .inspect_err(|err| {
                                warn!(
                                    "Failed to send upsert action for deployment {}: {}",
                                    deployment_ref, err
                                );
                            })
                            .ok();
                        }
                    })
                    .run()
                    .await;

                continue_after!(
                    options.auto_cycle_duration(),
                    gateway_instances_rx.changed(),
                    current_service_refs_rx.changed()
                );
            }
        });
}
