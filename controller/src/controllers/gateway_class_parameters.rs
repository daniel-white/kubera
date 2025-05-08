use crate::api::v1alpha1::GatewayClassParameters;
use crate::controllers::gateway_class::GatewayClassState;
use derive_builder::Builder;
use derive_getters::Getters;
use futures::StreamExt;
use futures_signals::signal::{Mutable, MutableSignalCloned, SignalExt};
use kube::runtime::controller::Action;
use kube::runtime::watcher::Config;
use kube::runtime::Controller;
use kube::{Api, Client};
use std::future::ready;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::spawn;
use tokio::task::JoinHandle;

#[derive(Builder, Clone, PartialEq, Getters, Debug)]
pub struct GatewayClassParametersState {}

pub type GatewayClassParametersStateSignal =
    MutableSignalCloned<Option<GatewayClassParametersState>>;

#[derive(Error, Debug)]
enum ControllerError {}

pub struct GatewayClassParametersController {
    client: Client,
    state: Arc<Mutable<Option<GatewayClassParametersState>>>,
    gateway_class_state: Arc<Mutable<Option<GatewayClassState>>>,
}

impl GatewayClassParametersController {
    pub fn new(
        client: Client,
        gateway_class_state: Arc<Mutable<Option<GatewayClassState>>>,
    ) -> Self {
        GatewayClassParametersController {
            client,
            state: Arc::new(Mutable::new(None)),
            gateway_class_state,
        }
    }

    pub fn state(&self) -> Arc<Mutable<Option<GatewayClassParametersState>>> {
        self.state.clone()
    }

    pub fn run(&self) -> JoinHandle<()> {
        let client = self.client.clone();
        let state = self.state.clone();
        let gateway_class_state = self.gateway_class_state.clone();

        spawn(async move {
            let mut current_task: Option<JoinHandle<()>> = None;
            let mut gateway_class_state_stream = gateway_class_state.signal_cloned().to_stream();

            loop {
                let event = gateway_class_state_stream.next().await;
                if let Some(current_task) = current_task.take() {
                    current_task.abort();
                }

                match event {
                    None => break,
                    Some(gateway_class_state) => {
                        let client = client.clone();
                        let state = state.clone();
                        let gateway_class_state = gateway_class_state.clone();
                        current_task = Some(spawn(async move {
                            loop {
                                if let Some(gateway_class_state) = gateway_class_state.as_ref() {
                                    if let Some(parameters_ref) = gateway_class_state.parameter_ref() {
                                        let parameters = Api::<GatewayClassParameters>::all(client.clone());
                                        let watcher_config = Config::default().fields(
                                            format!("metadata.name={}", parameters_ref.name())
                                                .as_str()
                                        );

                                        Controller::new(parameters, watcher_config)
                                            .run(
                                                async |parameters, state| {
                                                    state.set_neq(Some(GatewayClassParametersState {}));
                                                    Ok(Action::requeue(Duration::from_secs(60)))
                                                },
                                                |_, _: &ControllerError, _| {
                                                    state.set_neq(None);
                                                    Action::requeue(Duration::from_secs(5))
                                                },
                                                Arc::new(state.clone()),
                                            )
                                            .for_each(|_| ready(()))
                                            .await
                                    } else {
                                        state.set_neq(None);
                                        tokio::time::sleep(Duration::from_secs(10)).await;
                                    }
                                } else {
                                    state.set_neq(None);
                                    tokio::time::sleep(Duration::from_secs(10)).await;
                                }
                            }
                        }));
                    }
                }
        }})
    }
}
