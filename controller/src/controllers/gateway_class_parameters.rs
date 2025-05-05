use crate::api::v1alpha1::GatewayClassParameters;
use crate::controllers::gateway_class::ControllerError;
use crate::controllers::state::{GatewayClassState, StateEvents};
use futures::StreamExt;
use kube::runtime::controller::Action;
use kube::runtime::watcher::Config;
use kube::runtime::Controller;
use kube::{Api, Client};
use log::info;
use std::future::ready;
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tokio::signal::ctrl_c;
use tokio::sync::mpsc::Sender;
use tokio::sync::watch::Receiver;

pub async fn watch_gateway_class_parameters(
    client: &Client,
    state_events_tx: Sender<StateEvents>,
    mut rx: Receiver<Option<GatewayClassState>>,
) {
    loop {
        let gateway_class_state = rx.borrow().clone();

        if let Some(gateway_class_state) = gateway_class_state.as_ref() {
            select! {
                _ = watch_gateway_class_parameters_impl(client, &state_events_tx, gateway_class_state) => {},
                res = rx.changed() => {
                    match res {
                        Ok(_) => {
                            continue;
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }
            }
        } else {
            select! {
                res = rx.changed() => {
                    match res {
                        Ok(_) => {
                            continue;
                        }
                        Err(_) => {
                            break;
                        }
                    }
                },
                _ = ctrl_c() => {
                    break;
                },
                _ = tokio::time::sleep(Duration::from_secs(10)) => {
                    continue;
                }
            }
        }
    }
}

async fn watch_gateway_class_parameters_impl(
    client: &Client,
    state_events_tx: &Sender<StateEvents>,
    gateway_class_state: &GatewayClassState,
) {
    match &gateway_class_state.parameter_ref() {
        Some(parameters_ref) => {
            info!(
                "Watching gateway class parameters: {}",
                parameters_ref.name()
            );

            let gateway_class_parameters = Api::<GatewayClassParameters>::all(client.clone());
            let watcher_config =
                Config::default().fields(&format!("metadata.name={}", parameters_ref.name()));

            Controller::new(gateway_class_parameters, watcher_config)
                .shutdown_on_signal()
                .run(
                    |parameters, _| ready(Ok(Action::requeue(Duration::from_secs(60)))),
                    |_: Arc<GatewayClassParameters>, _: &ControllerError, _| {
                        Action::requeue(Duration::from_secs(5))
                    },
                    Arc::new(()),
                )
                .for_each(|_| ready(()))
                .await;
        }
        None => {
            info!("No parameter refs found, skipping watch");
            return;
        }
    }
}
