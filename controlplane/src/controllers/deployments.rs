use crate::constants::MANAGED_BY_LABEL_QUERY;
use crate::controllers::Ref;
use crate::sync::state::Receiver;
use derive_builder::Builder;
use futures::StreamExt;
use getset::Getters;
use k8s_openapi::api::apps::v1::Deployment;
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
pub struct DeploymentsState {
    #[builder(default)]
    #[getset(get = "pub")]
    deployments: HashMap<Ref, ()>,
}

#[derive(Error, Debug)]
pub enum ControllerError {}

struct Context {
    client: Client,
    state_tx: crate::sync::state::Sender<DeploymentsState>,
}

async fn reconcile(
    deployments: Arc<Deployment>,
    ctx: Arc<Context>,
) -> Result<Action, ControllerError> {
    let mut new_state = ctx.state_tx.current();

    let deployments_ref = Ref::new_builder()
        .name(
            deployments
                .metadata
                .name
                .clone()
                .expect("Deployment must have a name"),
        )
        .namespace(deployments.metadata.namespace.clone())
        .build()
        .expect("Failed to build Ref for Deployment");

    match &deployments.metadata.deletion_timestamp {
        None => {
            new_state.deployments.insert(deployments_ref, ());
        }
        _ => {
            new_state.deployments.remove(&deployments_ref);
        }
    }

    ctx.state_tx.replace(new_state);
    Ok(Action::requeue(Duration::from_secs(60)))
}

fn error_policy(_: Arc<Deployment>, error: &ControllerError, _: Arc<Context>) -> Action {
    Action::requeue(Duration::from_secs(5))
}

pub async fn controller(
    client: &Client,
) -> Result<(tokio::task::JoinHandle<()>, Receiver<DeploymentsState>), ControllerError> {
    let deployments = Api::<Deployment>::all(client.clone());

    let client = client.clone();
    let (state_tx, state_rx) =
        crate::sync::state::channel::<DeploymentsState>(DeploymentsState::default());

    let join_handle = tokio::spawn(async move {
        Controller::new(
            deployments.clone(),
            Config::default().labels(MANAGED_BY_LABEL_QUERY),
        )
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
