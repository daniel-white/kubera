use crate::constants::MANAGED_BY_LABEL_QUERY;
use crate::controllers::Ref;
use crate::sync::state::Receiver;
use derive_builder::Builder;
use futures::StreamExt;
use getset::Getters;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::runtime::Controller;
use kube::runtime::controller::Action;
use kube::runtime::watcher::Config;
use kube::{Api, Client};
use std::collections::HashMap;
use std::future::ready;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::task::JoinSet;

#[derive(Builder, Getters, Default, Clone, PartialEq, Debug)]
pub struct ConfigMapsState {
    #[builder(default)]
    #[getset(get = "pub")]
    config_maps: HashMap<Ref, ()>,
}

#[derive(Error, Debug)]
pub enum ControllerError {}

struct Context {
    client: Client,
    state_tx: crate::sync::state::Sender<ConfigMapsState>,
}

async fn reconcile(
    config_maps: Arc<ConfigMap>,
    ctx: Arc<Context>,
) -> Result<Action, ControllerError> {
    let mut new_state = ctx.state_tx.current();

    let config_maps_ref = Ref::new_builder()
        .name(
            config_maps
                .metadata
                .name
                .clone()
                .expect("ConfigMap must have a name"),
        )
        .namespace(config_maps.metadata.namespace.clone())
        .build()
        .expect("Failed to build Ref for ConfigMap");

    match &config_maps.metadata.deletion_timestamp {
        None => {
            new_state.config_maps.insert(config_maps_ref, ());
        }
        _ => {
            new_state.config_maps.remove(&config_maps_ref);
        }
    }

    ctx.state_tx.replace(new_state);
    Ok(Action::requeue(Duration::from_secs(60)))
}

fn error_policy(_: Arc<ConfigMap>, error: &ControllerError, _: Arc<Context>) -> Action {
    Action::requeue(Duration::from_secs(5))
}

pub async fn spawn_controller(
    join_set: &mut JoinSet<()>,
    client: &Client,
) -> Result<Receiver<ConfigMapsState>, ControllerError> {
    let config_maps = Api::<ConfigMap>::all(client.clone());

    let client = client.clone();
    let (state_tx, state_rx) =
        crate::sync::state::channel::<ConfigMapsState>(ConfigMapsState::default());

    join_set.spawn(async move {
        Controller::new(
            config_maps.clone(),
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

    Ok(state_rx)
}
