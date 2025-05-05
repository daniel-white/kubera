use crate::controllers::gateway_class::ControllerError;
use crate::controllers::state::{GatewayClassState, StateEvents};
use futures::StreamExt;
use gateway_api::apis::standard::gateways::Gateway;
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

pub async fn watch_gateways(client: &Client, state_events_tx: Sender<StateEvents>,  mut rx: Receiver<Option<GatewayClassState>>) {
    loop {
        let gateway_class_state = rx.borrow().clone();

        if let Some(gateway_class_state) = gateway_class_state.as_ref() {
            select! {
                _ = watch_gateway_impl(client, &state_events_tx, gateway_class_state) => {},
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

async fn watch_gateway_impl(client: &Client, state_events_tx: &Sender<StateEvents>, gateway_class_state: &GatewayClassState) {
    info!("Watching for gateways {}", gateway_class_state.name());

    let gateways = Api::<Gateway>::all(client.clone());
    let watcher_config = Config::default().fields(&format!(
        "spec.gatewayClassName={}",
        gateway_class_state.name()
    ));

    Controller::new(gateways, watcher_config)
        .shutdown_on_signal()
        .run(
            |_, _| ready(Ok(Action::requeue(Duration::from_secs(60)))),
            |_: Arc<Gateway>, _: &ControllerError, _| Action::requeue(Duration::from_secs(5)),
            Arc::new(()),
        )
        .for_each(|_| ready(()))
        .await;
}
