use crate::objects::{ObjectRef, ObjectTracker, Objects, SyncObjectAction};
use crate::sync_objects;
use derive_builder::Builder;
use gateway_api::apis::standard::gateways::Gateway;
use gtmpl_derive::Gtmpl;
use k8s_openapi::api::core::v1::Service;
use kube::Client;
use kubera_core::sync::signal::Receiver;
use kubera_core::continue_after;
use std::collections::HashSet;
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
    gateways: &Receiver<Objects<Gateway>>,
) {
    let tx = sync_objects!(join_set, Service, client, TemplateValues, TEMPLATE);
    generate_gateway_services(join_set, tx, gateways);
}

fn generate_gateway_services(
    join_set: &mut JoinSet<()>,
    tx: Sender<SyncObjectAction<TemplateValues>>,
    gateways: &Receiver<Objects<Gateway>>,
) {
    let mut gateways = gateways.clone();
    let tracker = ObjectTracker::new();

    join_set.spawn(async move {
        loop {
            let services: Vec<_> = gateways
                .current()
                .iter()
                .map(|(gateway_ref, _, _)| {
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

                    (service_ref, template_values)
                })
                .collect();

            let service_refs: HashSet<_> = services.iter().map(|(ref_, _)| ref_.clone()).collect();

            let deleted_refs = tracker.reconcile(service_refs);
            for deleted_ref in deleted_refs {
                tx.send(SyncObjectAction::Delete(deleted_ref))
                    .expect("Failed to send delete action");
            }

            for (service_ref, template_values) in services.into_iter() {
                tx.send(SyncObjectAction::Upsert(service_ref, template_values))
                    .expect("Failed to send upsert action");
            }

            continue_after!(Duration::from_secs(60), gateways.changed());
        }
    });
}
