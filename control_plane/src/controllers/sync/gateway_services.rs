use crate::controllers::instances::InstanceRole;
use crate::controllers::transformers::GatewayInstanceConfiguration;
use crate::kubernetes::objects::{ObjectRef, SyncObjectAction};
use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use crate::{sync_objects, watch_objects};
use derive_builder::Builder;
use gtmpl_derive::Gtmpl;
use k8s_openapi::api::core::v1::Service;
use kube::runtime::watcher::Config;
use kubera_core::continue_after;
use kubera_core::sync::signal::Receiver;
use kubera_core::task::Builder as TaskBuilder;
use kubera_macros::await_ready;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tracing::debug;

const TEMPLATE: &str = include_str!("./templates/gateway_service.kubernetes-helm-yaml");

#[derive(Clone, Builder, Debug, Gtmpl)]
#[builder(setter(into))]
struct TemplateValues {
    gateway_name: String,
}

pub fn sync_gateway_services(
    options: Arc<Options>,
    task_builder: &TaskBuilder,
    kube_client_rx: &Receiver<KubeClientCell>,
    instance_role_rx: &Receiver<InstanceRole>,
    gateway_instances_rx: &Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
) {
    let (tx, current_refs_rx) = sync_objects!(
        options,
        task_builder,
        Service,
        kube_client_rx,
        instance_role_rx,
        TemplateValues,
        TEMPLATE
    );
    generate_gateway_services(
        options,
        task_builder,
        tx,
        current_refs_rx,
        gateway_instances_rx,
    );
}

fn generate_gateway_services(
    options: Arc<Options>,
    task_builder: &TaskBuilder,
    tx: Sender<SyncObjectAction<TemplateValues, Service>>,
    service_refs_rx: Receiver<HashSet<ObjectRef>>,
    gateway_instances_rx: &Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
) {
    let gateway_instances_rx = gateway_instances_rx.clone();

    task_builder
        .new_task(stringify!(generate_gateway_services))
        .spawn(async move {
            loop {
                await_ready!(gateway_instances_rx, service_refs_rx)
                    .and_then(async |gateway_instances, service_refs| {
                        let desired_services: Vec<_> = gateway_instances
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

                        let desired_service_refs: HashSet<_> = desired_services
                            .iter()
                            .map(|(ref_, _, _, _)| ref_.clone())
                            .collect();

                        let deleted_refs = service_refs.difference(&desired_service_refs);
                        for deleted_ref in deleted_refs {
                            tx.send(SyncObjectAction::Delete(deleted_ref.clone()))
                                .expect("Failed to send delete action");
                        }

                        for (service_ref, gateway_ref, template_values, service_overrides) in
                            desired_services
                        {
                            tx.send(SyncObjectAction::Upsert(
                                service_ref,
                                gateway_ref.clone(),
                                template_values,
                                Some(service_overrides.clone()),
                            ))
                            .expect("Failed to send upsert action");
                        }
                    })
                    .run()
                    .await;

                continue_after!(
                    options.auto_cycle_duration(),
                    gateway_instances_rx.changed(),
                    service_refs_rx.changed()
                );
            }
        });
}
