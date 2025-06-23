use crate::objects::{ObjectRef, ObjectTracker, Objects, SyncObjectAction};
use crate::sync_objects;
use derive_builder::Builder;
use gateway_api::apis::standard::gateways::Gateway;
use gtmpl_derive::Gtmpl;
use k8s_openapi::api::apps::v1::Deployment;
use kube::Client;
use kubera_core::sync::signal::Receiver;
use kubera_core::{continue_after, continue_on};
use std::collections::HashSet;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::broadcast::{channel, Sender};

const TEMPLATE: &str = include_str!("./templates/gateway_deployment.kubernetes-helm-yaml");

#[derive(Clone, Builder, Debug, Gtmpl)]
#[builder(setter(into))]
struct TemplateValues {
    gateway_name: String,
    controlplane_host: String,
    configmap_name: String,
}

pub fn sync_gateway_deployments(client: &Client, gateways: &Receiver<Objects<Gateway>>) {
    let (tx, rx) = channel(1);
    sync_objects!(Deployment, client, rx, TemplateValues, TEMPLATE);
    generate_gateway_deployments(tx, gateways);
}

fn generate_gateway_deployments(
    tx: Sender<SyncObjectAction<TemplateValues>>,
    gateways: &Receiver<Objects<Gateway>>,
) {
    let mut gateways = gateways.clone();
    let tracker = ObjectTracker::new();

    spawn(async move {
        loop {
            let deployments: Vec<_> = gateways
                .current()
                .iter()
                .map(|(gateway_ref, _, _)| {
                    let deployment_ref = ObjectRef::new_builder()
                        .of_kind::<Deployment>()
                        .namespace(gateway_ref.namespace().clone())
                        .name(gateway_ref.name())
                        .build()
                        .expect("Failed to build ObjectRef for Deployment");

                    let template_values = TemplateValuesBuilder::default()
                        .gateway_name(gateway_ref.name())
                        .controlplane_host("hello world")
                        .configmap_name(format!("{}-config", gateway_ref.name()))
                        .build()
                        .expect("Failed to build TemplateValues");

                    (deployment_ref, template_values)
                })
                .collect();

            let deployment_refs: HashSet<_> =
                deployments.iter().map(|(ref_, _)| ref_.clone()).collect();

            let deleted_refs = tracker.reconcile(deployment_refs);
            for deleted_ref in deleted_refs {
                tx.send(SyncObjectAction::Delete(deleted_ref))
                    .expect("Failed to send delete action");
            }

            for (deployment_ref, template_values) in deployments.into_iter() {
                tx.send(SyncObjectAction::Upsert(deployment_ref, template_values))
                    .expect("Failed to send upsert action");
            }

            continue_after!(Duration::from_secs(60), gateways.changed());
        }
    });
}
