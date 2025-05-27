use crate::api::constants::{GATEWAY_CLASS_PARAMETERS_CRD_KIND, GROUP};
use crate::controllers::Ref;
use crate::sync::state::{Receiver, Sender, channel};
use derive_builder::Builder;
use futures::StreamExt;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use kube::api::ListParams;
use kube::{
    Api, Client, ResourceExt,
    runtime::{Controller, controller::Action, watcher::Config},
};
use std::{future::ready, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::spawn;
use tokio::task::JoinHandle;

#[derive(Builder, Clone, PartialEq, Debug)]
pub struct GatewayClassState {
    #[builder(setter(into))]
    name: String,
    #[builder(default)]
    parameter_ref: Option<Ref>,
}

impl GatewayClassState {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn parameter_ref(&self) -> Option<Ref> {
        self.parameter_ref.clone()
    }
}

#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("error querying GatewayClass CRD: `{0}`; are the Gateway API CRDs installed?")]
    CRDNotFound(#[source] kube::Error),
}

struct Context {
    client: Client,
    state_tx: Sender<Option<GatewayClassState>>,
}

pub async fn controller(
    client: &Client,
) -> Result<(JoinHandle<()>, Receiver<Option<GatewayClassState>>), ControllerError> {
    let gateway_class = Api::<GatewayClass>::all(client.clone());
    gateway_class
        .list(&ListParams::default().limit(1))
        .await
        .map_err(ControllerError::CRDNotFound)?;

    let client = client.clone();
    let (state_tx, state_rx) = channel::<Option<GatewayClassState>>(None);

    let join_handle = spawn(async move {
        Controller::new(gateway_class, Config::default().any_semantic())
            .shutdown_on_signal()
            .run(
                reconcile,
                error_policy,
                Arc::new(Context { client, state_tx }),
            )
            .filter_map(|x| async move { Some(x) })
            .for_each(|_| ready(()))
            .await;
    });

    Ok((join_handle, state_rx))
}

async fn reconcile(
    gateway_class: Arc<GatewayClass>,
    ctx: Arc<Context>,
) -> Result<Action, ControllerError> {
    let new_state = match gateway_class.metadata.deletion_timestamp {
        None => {
            let parameters_ref = match &gateway_class.spec.parameters_ref {
                Some(param_ref)
                    if param_ref.kind == GATEWAY_CLASS_PARAMETERS_CRD_KIND
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

            GatewayClassStateBuilder::default()
                .name(gateway_class.name_any())
                .parameter_ref(parameters_ref)
                .build()
                .ok()
        }
        Some(_) => None,
    };

    ctx.state_tx.replace(new_state);

    Ok(Action::requeue(Duration::from_secs(60)))
}

fn error_policy(_: Arc<GatewayClass>, error: &ControllerError, _: Arc<Context>) -> Action {
    Action::requeue(Duration::from_secs(5))
}
