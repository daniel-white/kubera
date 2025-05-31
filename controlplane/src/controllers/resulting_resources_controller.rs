use crate::controllers::desired_resources_controller::{ControllerError, DesiredResources};
use crate::sync::state::Receiver;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{ConfigMap, Service};
use kube::Api;
use kube::Client;
use log::info;
use tokio::signal;
use tokio::task::JoinSet;

pub async fn spawn_controller(
    join_set: &mut JoinSet<()>,
    client: &Client,
    mut desired_resources: Receiver<Option<DesiredResources>>,
) -> Result<(), ControllerError> {
    let client = client.clone();
    join_set.spawn(async move {
        loop {
            if let Some(desired_resources) = desired_resources.current() {
                for namespaced_resources in desired_resources.namespaced() {
                    info!(
                        "Processing resources in namespace: {}",
                        namespaced_resources.namespace()
                    );

                    let config_maps = Api::<ConfigMap>::namespaced(
                        client.clone(),
                        namespaced_resources.namespace(),
                    );
                    let deployments = Api::<Deployment>::namespaced(
                        client.clone(),
                        namespaced_resources.namespace(),
                    );
                    let services = Api::<Service>::namespaced(
                        client.clone(),
                        namespaced_resources.namespace(),
                    );
                }
            }
            // for namespaced_resources in desired_resources.current()..iter() {
            //     let namespace = &namespaced_resources.namespace;
            //     let cm = Api::<ConfigMap>::namespaced(client.clone(), namespace);
            //     let pp = PostParams::default();
            //
            //     if let Some(resources) = namespaced_resources.current() {
            //         for resource in resources.config_maps().iter() {
            //             match resource {
            //                 DesiredResource::Create(config_map) => {
            //                     if let Err(e) = cm.create(&pp, config_map).await {
            //                         warn!(
            //                             "Failed to create ConfigMap in namespace {}: {}",
            //                             namespace, e
            //                         );
            //                     }
            //                 }
            //                 DesiredResource::Delete(key) => {
            //                     if let Err(e) = cm.delete(&key.name, &DeleteParams::default()).await
            //                     {
            //                         warn!(
            //                             "Failed to delete ConfigMap in namespace {}: {}",
            //                             namespace, e
            //                         );
            //                     }
            //                 }
            //                 DesiredResource::Patch(config_map) => {
            //                     // if let Err(e) =
            //                     //     cm.patch(&config_map.metadata.name, &pp, config_map).await
            //                     // {
            //                     //     warn!("Failed to patch ConfigMap in namespace {}: {}", namespace, e);
            //                     // }
            //                 }
            //             }
            //         }
            //     }
            // }
            tokio::select! {
                _ = desired_resources.changed() => {
                    continue;
                },
                _ = signal::ctrl_c() => {
                    // Handle graceful shutdown
                    break;
                },
            }
        }
    });

    Ok(())
}
