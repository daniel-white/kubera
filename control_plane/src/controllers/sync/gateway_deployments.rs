use crate::controllers::instances::InstanceRole;
use crate::controllers::transformers::GatewayInstanceConfiguration;
use crate::kubernetes::objects::{ObjectRef, SyncObjectAction};
use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use crate::{sync_objects, watch_objects};
use derive_builder::Builder;
use gtmpl_derive::Gtmpl;
use k8s_openapi::api::apps::v1::Deployment;
use kube::runtime::watcher::Config;
use kubera_core::continue_after;
use kubera_core::sync::signal::Receiver;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tokio::task::JoinSet;

const TEMPLATE: &str = include_str!("./templates/gateway_deployment.kubernetes-helm-yaml");

#[derive(Clone, Builder, Debug, Gtmpl)]
#[builder(setter(into))]
struct TemplateValues {
    gateway_name: String,
    configmap_name: String,
    image_pull_policy: String,
}

pub fn sync_gateway_deployments(
    options: Arc<Options>,
    join_set: &mut JoinSet<()>,
    kube_client: &Receiver<Option<KubeClientCell>>,
    instance_role: &Receiver<InstanceRole>,
    gateway_instances: &Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
) {
    let (tx, current_refs) = sync_objects!(
        options,
        join_set,
        Deployment,
        kube_client,
        instance_role,
        TemplateValues,
        TEMPLATE
    );
    generate_gateway_deployments(options, join_set, tx, current_refs, gateway_instances);
}

fn generate_gateway_deployments(
    options: Arc<Options>,
    join_set: &mut JoinSet<()>,
    tx: Sender<SyncObjectAction<TemplateValues, Deployment>>,
    current_refs_rx: Receiver<Option<HashSet<ObjectRef>>>,
    gateway_instances: &Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
) {
    let gateway_instances = gateway_instances.clone();

    join_set.spawn(async move {
        loop {
            let intended = gateway_instances.current();
            let intended: Vec<_> = intended
                .iter()
                .map(|(gateway_ref, instance)| {
                    let deployment_ref = ObjectRef::new_builder()
                        .of_kind::<Deployment>()
                        .namespace(gateway_ref.namespace().clone())
                        .name(gateway_ref.name())
                        .build()
                        .expect("Failed to build ObjectRef for Deployment");

                    let image_pull_policy: &'static str = instance.image_pull_policy().into();

                    let template_values = TemplateValuesBuilder::default()
                        .gateway_name(gateway_ref.name())
                        .configmap_name(format!("{}-config", gateway_ref.name()))
                        .image_pull_policy(image_pull_policy)
                        .build()
                        .expect("Failed to build TemplateValues");

                    (
                        deployment_ref,
                        gateway_ref,
                        template_values,
                        instance.deployment_overrides(),
                    )
                })
                .collect();

            if let Some(current_refs) = current_refs_rx.current().as_ref() {
                let intended_refs: HashSet<_> = intended
                    .iter()
                    .map(|(ref_, _, _, _)| ref_.clone())
                    .collect();

                let deleted_refs = current_refs.difference(&intended_refs);
                for deleted_ref in deleted_refs {
                    tx.send(SyncObjectAction::Delete(deleted_ref.clone()))
                        .expect("Failed to send delete action");
                }
            }

            for (deployment_ref, gateway_ref, template_values, deployment_overrides) in
                intended.into_iter()
            {
                tx.send(SyncObjectAction::Upsert(
                    deployment_ref,
                    gateway_ref.clone(),
                    template_values,
                    Some(deployment_overrides.clone()),
                ))
                .expect("Failed to send upsert action");
            }

            continue_after!(
                options.auto_cycle_duration(),
                gateway_instances.changed(),
                current_refs_rx.changed()
            );
        }
    });
}
