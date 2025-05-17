use crate::api::v1alpha1::GatewayClassParameters;
use crate::controllers::gateway_class::GatewayClassState;
use crate::sync::state::{Receiver, Sender, channel};
use derive_builder::Builder;
use derive_getters::Getters;
use futures::StreamExt;
use kube::runtime::Controller;
use kube::runtime::controller::Action;
use kube::runtime::watcher::Config;
use kube::{Api, Client};
use std::future::ready;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::task::JoinHandle;
use tokio::{select, spawn};

#[derive(Builder, Clone, PartialEq, Getters, Debug)]
pub struct GatewayClassParametersState {}

#[derive(Error, Debug)]
enum ControllerError {}

pub fn controller(
    client: &Client,
    gateway_class_state_rx: &Receiver<Option<GatewayClassState>>,
) -> (
    JoinHandle<()>,
    Receiver<Option<GatewayClassParametersState>>,
) {
    let client = client.clone();
    let mut gateway_class_state_rx = gateway_class_state_rx.clone();
    let (state_tx, state_rx) = channel::<Option<GatewayClassParametersState>>(None);


    let join_handle = spawn(async move {
        loop {
            let gateway_class_state = gateway_class_state_rx.current();

            match gateway_class_state.as_ref() {
                None => {
                    gateway_class_state_rx.changed().await;
                    continue;
                }
                Some(gateway_class_state) => match gateway_class_state.parameter_ref() {
                    None => {
                        gateway_class_state_rx.changed().await;
                        continue;
                    }
                    Some(parameters_ref) => {
                        let parameters_api = Api::<GatewayClassParameters>::all(client.clone());

                        if parameters_api
                            .get_metadata_opt(parameters_ref.name())
                            .await
                            .ok()
                            .flatten()
                            .is_none()
                        {
                            state_tx.replace(None);
                            select! {
                                _ = tokio::time::sleep(Duration::from_secs(60)) => {
                                    continue;
                                },
                                _ = gateway_class_state_rx.changed() => {
                                    continue;
                                },
                            }
                        }

                        let watcher_config = Config::default()
                            .fields(format!("metadata.name={}", parameters_ref.name()).as_str());

                        select! {
                            _ = Controller::new(parameters_api, watcher_config)
                                .shutdown_on_signal()
                                .run(
                                    async |parameters, state_tx| {
                                        state_tx.replace(Some(GatewayClassParametersState {}));
                                        Ok(Action::requeue(Duration::from_secs(60)))
                                    },
                                    |_, _: &ControllerError, _| {
                                        state_tx.replace(None);
                                        Action::requeue(Duration::from_secs(5))
                                    },
                                    Arc::new(state_tx.clone()),
                                )
                                .for_each(|_| ready(()))
                                 => {
                                    // Handle the controlplane's shutdown signal
                                    break;
                                },
                            _ = gateway_class_state_rx.changed() => {
                                continue;
                            },
                        }
                    }
                },
            }
        }
    });

    (join_handle, state_rx)
}
