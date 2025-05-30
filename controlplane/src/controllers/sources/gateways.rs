use crate::constants::{GATEWAY_PARAMETERS_CRD_KIND, GROUP};
use crate::controllers::Ref;
use crate::controllers::sources::gateway_class::GatewayClassState;
use crate::sync::state::Receiver;
use derive_builder::Builder;
use futures::StreamExt;
use gateway_api::apis::standard::gateways::Gateway as GatewayCrd;
use getset::Getters;
use kube::runtime::Controller;
use kube::runtime::controller::Action;
use kube::runtime::watcher::Config;
use kube::{Api, Client};
use romap::RoMap;
use std::collections::HashMap;
use std::future::ready;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::select;
use tokio::task::JoinSet;

#[derive(Builder, Default, Clone, PartialEq, Debug)]
pub struct GatewaysState {
    #[builder(default)]
    gateways: HashMap<Ref, Gateway>,
}

#[derive(Builder, Getters, Clone, PartialEq, Debug)]
pub struct Gateway {
    #[builder(default)]
    #[getset(get = "pub")]
    parameter_ref: Option<Ref>,
}

impl Gateway {
    pub fn new_builder() -> GatewayBuilder {
        GatewayBuilder::default()
    }
}

impl GatewaysState {
    pub fn gateways(&self) -> impl RoMap<Ref, Gateway> {
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
    gateway_class_name: String,
    state_tx: crate::sync::state::Sender<GatewaysState>,
}

async fn reconcile(gateway: Arc<GatewayCrd>, ctx: Arc<Context>) -> Result<Action, ControllerError> {
    let mut new_state = ctx.state_tx.current();

    let gateway_ref = Ref::new_builder()
        .name(
            gateway
                .metadata
                .name
                .clone()
                .expect("Gateway must have a name"),
        )
        .namespace(gateway.metadata.namespace.clone())
        .build()
        .expect("Failed to build Ref for Gateway");

    match (
        &gateway.spec.gateway_class_name == &ctx.gateway_class_name,
        &gateway.metadata.deletion_timestamp,
    ) {
        (true, None) => {
            let parameter_ref = match gateway
                .spec
                .infrastructure
                .as_ref()
                .and_then(|infra| infra.parameters_ref.as_ref())
            {
                Some(param_ref)
                    if param_ref.kind == GATEWAY_PARAMETERS_CRD_KIND
                        && param_ref.group == GROUP =>
                {
                    Ref::new_builder()
                        .name(&param_ref.name)
                        .namespace(None)
                        .build()
                        .ok()
                }
                _ => None,
            };

            let gateway_state = Gateway::new_builder()
                .parameter_ref(parameter_ref)
                .build()
                .expect("Failed to build GatewayState");
            new_state.gateways.insert(gateway_ref, gateway_state);
        }
        _ => {
            new_state.gateways.remove(&gateway_ref);
        }
    }

    ctx.state_tx.replace(new_state);
    Ok(Action::requeue(Duration::from_secs(60)))
}

fn error_policy(_: Arc<GatewayCrd>, error: &ControllerError, _: Arc<Context>) -> Action {
    Action::requeue(Duration::from_secs(5))
}

pub async fn spawn_controller(
    join_set: &mut JoinSet<()>,
    client: &Client,
    gateway_class_state_rx: &Receiver<Option<GatewayClassState>>,
) -> Result<Receiver<GatewaysState>, ControllerError> {
    let gateways = Api::<GatewayCrd>::all(client.clone());

    gateways
        .list(&kube::api::ListParams::default().limit(1))
        .await
        .map_err(ControllerError::CRDNotFound)?;

    let client = client.clone();
    let mut gateway_class_state_rx = gateway_class_state_rx.clone();
    let (state_tx, state_rx) =
        crate::sync::state::channel::<GatewaysState>(GatewaysState::default());

    join_set.spawn(async move {
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
                                gateway_class_name,
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

    Ok(state_rx)
}
