use crate::controllers::desired_resources_controller::{ControllerError, DesiredResources};
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use kube::Api;
use kube::Client;
use kube::api::PostParams;
use kubera_core::select_continue;
use kubera_core::sync::signal::Receiver;
use tokio::task::JoinSet;
use tracing::warn;

pub async fn spawn_controller(
    join_set: &mut JoinSet<()>,
    client: &Client,
    mut desired_resources: Receiver<Option<DesiredResources>>,
) -> Result<(), ControllerError> {
    let client = client.clone();
    join_set.spawn(async move {
        loop {
            if let Some(desired_resources) = desired_resources.current().as_ref() {
                let gateway_classes = Api::<GatewayClass>::all(client.clone());

                for (ref_, gateway_class) in desired_resources.gateway_classes() {
                    let _ = gateway_classes
                        .replace_status(
                            ref_.name(),
                            &PostParams::default(),
                            serde_json::to_vec(gateway_class).unwrap(),
                        )
                        .await
                        .inspect_err(|e| {
                            warn!("Failed to update GatewayClass status for {:?}: {}", ref_, e);
                        });
                }

                // for namespaced_resources in desired_resources.namespaced() {
                //     info!(
                //         "Processing resources in namespace: {}",
                //         namespaced_resources.namespace()
                //     );
                //
                //     let config_maps = Api::<ConfigMap>::namespaced(
                //         client.clone(),
                //         namespaced_resources.namespace(),
                //     );
                //     let deployments = Api::<Deployment>::namespaced(
                //         client.clone(),
                //         namespaced_resources.namespace(),
                //     );
                //     let services = Api::<Service>::namespaced(
                //         client.clone(),
                //         namespaced_resources.namespace(),
                //     );
                //
                //     for config_map in namespaced_resources.config_maps() {
                //         match config_map {
                //             DesiredResource::Create(cm) => {
                //                 if let Err(e) = config_maps.create(&PostParams::default(), cm).await
                //                 {
                //                     warn!(
                //                         "Failed to create ConfigMap in namespace {}: {}",
                //                         namespaced_resources.namespace(),
                //                         e
                //                     );
                //                 }
                //             }
                //             DesiredResource::Delete(key) => {
                //                 if let Err(e) = config_maps
                //                     .delete(&key.name, &DeleteParams::default())
                //                     .await
                //                 {
                //                     warn!(
                //                         "Failed to delete ConfigMap in namespace {}: {}",
                //                         namespaced_resources.namespace(),
                //                         e
                //                     );
                //                 }
                //             }
                //             DesiredResource::Patch(name, patch) => {
                //                 if let Err(e) = config_maps
                //                     .patch(name, &PatchParams::default(), patch)
                //                     .await
                //                 {
                //                     warn!(
                //                         "Failed to patch ConfigMap in namespace {}: {}",
                //                         namespaced_resources.namespace(),
                //                         e
                //                     );
                //                 }
                //             }
                //         }
                //     }
                // }
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
            select_continue!(desired_resources.changed())
        }
    });

    Ok(())
}
