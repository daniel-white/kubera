use crate::controllers::gateway_class::GatewayClassState;
use crate::sync::state::Receiver;
use derive_builder::Builder;
use futures::StreamExt;
use gateway_api::apis::standard::gateways::Gateway;
use kube::runtime::controller::Action;
use kube::runtime::watcher::Config;
use kube::runtime::Controller;
use kube::{Api, Client};
use std::future::ready;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::select;

#[derive(Builder, Default, Clone, PartialEq, Debug)]
pub struct GatewaysState {
    #[builder(default)]
    gateways: Vec<()>,
}

impl GatewaysState {
    pub fn gateways(&self) -> &[()] {
        &self.gateways
    }
}

#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("error querying Gateway CRD: `{0}`; are the Gateway API CRDs installed?")]
    CRDNotFound(#[source] kube::Error),
}

struct Context {
    client: Client,
    state_tx: crate::sync::state::Sender<GatewaysState>,
}

async fn reconcile(gateway: Arc<Gateway>, ctx: Arc<Context>) -> Result<Action, ControllerError> {
    ctx.state_tx.replace(GatewaysState { gateways: vec![()] });
    Ok(Action::requeue(Duration::from_secs(60)))
}

fn error_policy(
    _: Arc<Gateway>,
    error: &ControllerError,
    _: Arc<Context>,
) -> Action {
    Action::requeue(Duration::from_secs(5))
}

pub async fn controller(
    client: &Client,
    gateway_class_state_rx: &Receiver<Option<GatewayClassState>>,
) -> Result<(tokio::task::JoinHandle<()>, Receiver<GatewaysState>), ControllerError> {
    let gateways = Api::<Gateway>::all(client.clone());

    gateways
        .list(&kube::api::ListParams::default().limit(1))
        .await
        .map_err(ControllerError::CRDNotFound)?;

    let client = client.clone();
    let mut gateway_class_state_rx = gateway_class_state_rx.clone();
    let (state_tx, state_rx) =
        crate::sync::state::channel::<GatewaysState>(GatewaysState::default());

    let join_handle = tokio::spawn(async move {
        loop {
            let gateway_class_state = gateway_class_state_rx.current();
            if let Some(gateway_class_name) = gateway_class_state.map(|s| s.name().to_string()) {
                let watcher_config = Config::default()
                    .fields(&format!("spec.gatewayClassName={}", gateway_class_name));
                select! {
                    _ = Controller::new(gateways.clone(), watcher_config)
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
