use crate::api::v1alpha1::Ref;
use futures::future::select;
use futures::StreamExt;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::{
    runtime::{controller::Action, watcher::Config, Controller}, Api,
    Client,
};
use log::info;
use std::{future::ready, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::select;
use tokio::signal::ctrl_c;
use tokio::sync::watch::{channel, Receiver, Sender};

#[derive(Debug, Clone)]
struct GatewayClassConfig {
    config_map_ref: Ref,
}

#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("Failed to get config map: {0}")]
    ConfigMapError(#[from] kube::Error),
}

pub async fn watch_gateway_class(client: &Client) {
    struct Context {
        tx: Sender<Option<GatewayClassConfig>>,
    }
    let (tx, rx) = channel(None);

    let x = watch_gateway_class_config(&client, rx);
    let gateway_classes = Api::<GatewayClass>::all(client.clone());
    let watcher_config = Config::default(); //.("spec.controllerName=whitefamily.in/kubera");

    let y = Controller::new(gateway_classes, watcher_config)
        .shutdown_on_signal()
        .run(
            |gateway_class, context| async move {
                if let Some(param_ref) = &gateway_class.spec.parameters_ref {
                    if param_ref.kind == "ConfigMap" && param_ref.group == "core" {
                        let config_map_ref = Ref {
                            name: param_ref.name.clone(),
                            namespace: param_ref.namespace.clone(),
                        };
                        let _ = context.tx.send(Some(GatewayClassConfig { config_map_ref }));
                    } else {
                        let _ = context.tx.send(None);
                    }
                } else {
                    let _ = context.tx.send(None);
                }

                Ok(Action::requeue(Duration::from_secs(60)))
            },
            |_: Arc<GatewayClass>, _: &ControllerError, _| Action::requeue(Duration::from_secs(5)),
            Arc::new(Context { tx }),
        )
        .for_each(|_| ready(()));

    futures::join!(x, y);
}

async fn watch_gateway_class_config(client: &Client, mut rx: Receiver<Option<GatewayClassConfig>>) {
    loop {
        let gateway_class_config = rx.borrow().clone();

        if let Some(gateway_class_config) = gateway_class_config {
            select! {
                _ = watch_gateway_class_config_map(client, gateway_class_config) => {
                },
                _ = rx.changed() => {
                    rx.mark_unchanged();
                }
            }
        } else {
            info!("No GatewayClassConfig found");
            select! {
                _ = rx.changed() => {
                    rx.mark_unchanged();
                },
                _ = ctrl_c() => {
                    info!("Received Ctrl-C, shutting down");
                    break;
                },
                _ = tokio::time::sleep(Duration::from_secs(10)) => {
                    info!("No GatewayClassConfig found, waiting for 10 seconds");
                }
            }
        }
    }
}

async fn watch_gateway_class_config_map(client: &Client, config: GatewayClassConfig) {
    let config_maps = match config.config_map_ref.namespace {
        Some(ref ns) => Api::<ConfigMap>::namespaced(client.clone(), ns),
        None => Api::<ConfigMap>::default_namespaced(client.clone()),
    };
    let watcher_config =
        Config::default().fields(&format!("metadata.name={}", config.config_map_ref.name));

    info!("Watching config map: {}", config.config_map_ref.name);

    Controller::new(config_maps, watcher_config)
        .shutdown_on_signal()
        .run(
            |config_map, _| ready(Ok(Action::requeue(Duration::from_secs(60)))),
            |_: Arc<ConfigMap>, _: &ControllerError, _| Action::requeue(Duration::from_secs(5)),
            Arc::new(()),
        )
        .for_each(|_| ready(()))
        .await;
}
