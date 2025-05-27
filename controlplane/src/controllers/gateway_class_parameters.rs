use crate::api::v1alpha1::GatewayClassParameters;
use crate::controllers::gateway_class::GatewayClassState;
use crate::sync::state::{Receiver, Sender, channel};
use derive_builder::Builder;
use derive_getters::Getters;
use futures::StreamExt;
use kube::api::ListParams;
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
pub struct GatewayClassParametersState {
    name: String,
}

#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("error querying GatewayClassParameters CRD: `{0}`; are the Kubera CRDs installed?")]
    CRDNotFound(#[source] kube::Error),
}

struct Context {
    client: Client,
    state_tx: Sender<Option<GatewayClassParametersState>>,
}

async fn reconcile(
    parameters: Arc<GatewayClassParameters>,
    ctx: Arc<Context>,
) -> Result<Action, ControllerError> {
    let new_state = match parameters.metadata.deletion_timestamp {
        None => None,
        Some(_) => Some(GatewayClassParametersState {
            name: parameters
                .metadata
                .resource_version
                .as_ref()
                .unwrap()
                .clone(),
        }),
    };

    ctx.state_tx.replace(new_state);

    Ok(Action::requeue(Duration::from_secs(60)))
}

fn error_policy(
    _: Arc<GatewayClassParameters>,
    error: &ControllerError,
    _: Arc<Context>,
) -> Action {
    Action::requeue(Duration::from_secs(5))
}

pub async fn controller(
    client: &Client,
    gateway_class_state_rx: &Receiver<Option<GatewayClassState>>,
) -> Result<
    (
        JoinHandle<()>,
        Receiver<Option<GatewayClassParametersState>>,
    ),
    ControllerError,
> {
    let gateway_class_parameters = Api::<GatewayClassParameters>::all(client.clone());

    gateway_class_parameters
        .list(&ListParams::default().limit(1))
        .await
        .map_err(ControllerError::CRDNotFound)?;

    let client = client.clone();
    let mut gateway_class_state_rx = gateway_class_state_rx.clone();
    let (state_tx, state_rx) = channel::<Option<GatewayClassParametersState>>(None);

    let join_handle = spawn(async move {
        loop {
            let gateway_class_state = gateway_class_state_rx.current();
            if let Some(parameters_ref) = gateway_class_state.and_then(|s| s.parameter_ref()) {
                let watcher_config = Config::default()
                    .fields(format!("metadata.name={}", parameters_ref.name()).as_str());
                select! {
                    _ = Controller::new(gateway_class_parameters.clone(), watcher_config)
                        .shutdown_on_signal()
                        .run(
                            reconcile,
                            error_policy,
                            Arc::new(Context {
                                client: client.clone(),
                                state_tx: state_tx.clone(),
                            }),
                        )
                        .for_each(|_| ready(())) => {
                            break;
                        },
                    changed_state = gateway_class_state_rx.changed() => {
                        match changed_state {
                            Some(_) => {
                                continue;
                            }
                            None => {
                                break;
                            }
                        }
                    },
                }
            }
        }
    });

    Ok((join_handle, state_rx))
}
