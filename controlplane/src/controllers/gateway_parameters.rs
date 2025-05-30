use crate::api::v1alpha1::GatewayParameters;
use crate::controllers::Ref;
use crate::sync::state::Receiver;
use derive_builder::Builder;
use futures::StreamExt;
use getset::Getters;
use kube::runtime::Controller;
use kube::runtime::controller::Action;
use kube::runtime::watcher::Config;
use kube::{Api, Client};
use std::collections::HashMap;
use std::future::ready;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

#[derive(Builder, Getters, Default, Clone, PartialEq, Debug)]
pub struct GatewayParametersState {
    #[builder(default)]
    #[getset(get = "pub")]
    parameters: HashMap<Ref, ()>,
}

#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("error querying GatewayParameters CRD: `{0}`; are the Kubera CRDs installed?")]
    CRDNotFound(#[source] kube::Error),
}

struct Context {
    client: Client,
    state_tx: crate::sync::state::Sender<GatewayParametersState>,
}

async fn reconcile(
    parameters: Arc<GatewayParameters>,
    ctx: Arc<Context>,
) -> Result<Action, ControllerError> {
    let mut new_state = ctx.state_tx.current();

    let parameters_ref = Ref::new_builder()
        .name(
            parameters
                .metadata
                .name
                .clone()
                .expect("GatewayParameters must have a name"),
        )
        .namespace(parameters.metadata.namespace.clone())
        .build()
        .expect("Failed to build Ref for GatewayParameters");

    match &parameters.metadata.deletion_timestamp {
        None => {
            new_state.parameters.insert(parameters_ref, ());
        }
        _ => {
            new_state.parameters.remove(&parameters_ref);
        }
    }

    ctx.state_tx.replace(new_state);
    Ok(Action::requeue(Duration::from_secs(60)))
}

fn error_policy(_: Arc<GatewayParameters>, error: &ControllerError, _: Arc<Context>) -> Action {
    Action::requeue(Duration::from_secs(5))
}

pub async fn controller(
    client: &Client,
) -> Result<
    (
        tokio::task::JoinHandle<()>,
        Receiver<GatewayParametersState>,
    ),
    ControllerError,
> {
    let parameters = Api::<GatewayParameters>::all(client.clone());

    parameters
        .list(&kube::api::ListParams::default().limit(1))
        .await
        .map_err(ControllerError::CRDNotFound)?;

    let client = client.clone();
    let (state_tx, state_rx) =
        crate::sync::state::channel::<GatewayParametersState>(GatewayParametersState::default());

    let join_handle = tokio::spawn(async move {
        Controller::new(parameters.clone(), Config::default())
            .shutdown_on_signal()
            .run(
                reconcile,
                error_policy,
                Arc::new(Context {
                    client: client.clone(),
                    state_tx: state_tx.clone(),
                }),
            )
            .filter_map(|x| async move { Some(x) })
            .for_each(|_| ready(()))
            .await;
    });

    Ok((join_handle, state_rx))
}
