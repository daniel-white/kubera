use crate::controllers::instances::InstanceRole;
use crate::controllers::transformers::GatewayInstanceConfiguration;
use crate::kubernetes::KubeClientCell;
use crate::kubernetes::objects::{ObjectRef, SyncObjectAction};
use crate::options::Options;
use crate::{sync_objects, watch_objects};
use derive_builder::Builder;
use gtmpl_derive::Gtmpl;
use k8s_openapi::api::core::v1::Service;
use kube::runtime::watcher::Config;
use kubera_core::continue_after;
use kubera_core::sync::signal::Receiver;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tokio::task::JoinSet;

const TEMPLATE: &str = include_str!("./templates/gateway_service.kubernetes-helm-yaml");

#[derive(Clone, Builder, Debug, Gtmpl)]
#[builder(setter(into))]
struct TemplateValues {
    gateway_name: String,
}

pub fn sync_gateway_services(
    options: Arc<Options>,
    join_set: &mut JoinSet<()>,
    kube_client: &Receiver<Option<KubeClientCell>>,
    instance_role: &Receiver<InstanceRole>,
    gateway_instances: &Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
) {
    let (tx, current_refs) = sync_objects!(
        options,
        join_set,
        Service,
        kube_client,
        instance_role,
        TemplateValues,
        TEMPLATE
    );
    generate_gateway_services(options, join_set, tx, current_refs, gateway_instances);
}

fn generate_gateway_services(
    options: Arc<Options>,
    join_set: &mut JoinSet<()>,
    tx: Sender<SyncObjectAction<TemplateValues, Service>>,
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
                    let service_ref = ObjectRef::new_builder()
                        .of_kind::<Service>()
                        .namespace(gateway_ref.namespace().clone())
                        .name(gateway_ref.name())
                        .build()
                        .expect("Failed to build ObjectRef for Service");

                    let template_values = TemplateValuesBuilder::default()
                        .gateway_name(gateway_ref.name())
                        .build()
                        .expect("Failed to build TemplateValues");

                    (
                        service_ref,
                        gateway_ref,
                        template_values,
                        instance.service_overrides(),
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

            for (service_ref, gateway_ref, template_values, service_overrides) in
                intended.into_iter()
            {
                tx.send(SyncObjectAction::Upsert(
                    service_ref,
                    gateway_ref.clone(),
                    template_values,
                    Some(service_overrides.clone()),
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
