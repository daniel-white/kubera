use crate::controllers::instances::InstanceRole;
use crate::controllers::transformers::GatewayInstanceConfiguration;
use crate::objects::{ObjectRef, ObjectTracker, SyncObjectAction};
use crate::sync_objects;
use derive_builder::Builder;
use gtmpl_derive::Gtmpl;
use k8s_openapi::api::core::v1::Service;
use kube::Client;
use kubera_core::continue_after;
use kubera_core::sync::signal::Receiver;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::sync::broadcast::Sender;
use tokio::task::JoinSet;

const TEMPLATE: &str = include_str!("./templates/gateway_service.kubernetes-helm-yaml");

#[derive(Clone, Builder, Debug, Gtmpl)]
#[builder(setter(into))]
struct TemplateValues {
    gateway_name: String,
}

pub fn sync_gateway_services(
    join_set: &mut JoinSet<()>,
    client: &Client,
    instance_role: &Receiver<InstanceRole>,
    gateway_instances: &Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
) {
    let tx = sync_objects!(
        join_set,
        Service,
        client,
        instance_role,
        TemplateValues,
        TEMPLATE
    );
    generate_gateway_services(join_set, tx, gateway_instances);
}

fn generate_gateway_services(
    join_set: &mut JoinSet<()>,
    tx: Sender<SyncObjectAction<TemplateValues, Service>>,
    gateway_instances: &Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
) {
    let gateway_instances = gateway_instances.clone();
    let tracker = ObjectTracker::new();

    join_set.spawn(async move {
        loop {
            let current_instances = gateway_instances.current();
            let services: Vec<_> = current_instances
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

            let service_refs: HashSet<_> = services
                .iter()
                .map(|(ref_, _, _, _)| ref_.clone())
                .collect();

            let deleted_refs = tracker.reconcile(service_refs);
            for deleted_ref in deleted_refs {
                tx.send(SyncObjectAction::Delete(deleted_ref))
                    .expect("Failed to send delete action");
            }

            for (service_ref, gateway_ref, template_values, service_overrides) in
                services.into_iter()
            {
                tx.send(SyncObjectAction::Upsert(
                    service_ref,
                    gateway_ref.clone(),
                    template_values,
                    Some(service_overrides.clone()),
                ))
                .expect("Failed to send upsert action");
            }

            continue_after!(Duration::from_secs(60), gateway_instances.changed());
        }
    });
}
