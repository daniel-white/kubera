use crate::api::constants::{GATEWAY_CLASS_PARAMETERS_CRD_KIND, GROUP};
use crate::controllers::state::{Ref, RefBuilder};
use crate::sync::state::{Receiver, Sender, channel};
use derive_builder::Builder;
use derive_getters::Getters;
use futures::StreamExt;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use kube::{
    Api, Client, ResourceExt,
    runtime::{Controller, controller::Action, watcher::Config},
};
use std::{future::ready, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::spawn;
use tokio::task::JoinHandle;

#[derive(Builder, Clone, Getters, PartialEq, Debug)]
pub struct GatewayClassState {
    #[builder(setter(into))]
    name: String,
    #[builder(default)]
    parameter_ref: Option<Ref>,
}

#[derive(Error, Debug)]
enum ControllerError {}

pub fn controller(client: &Client) -> (JoinHandle<()>, Receiver<Option<GatewayClassState>>) {
    let client = client.clone();
    let (state_tx, state_rx) = channel::<Option<GatewayClassState>>(None);

    let join_handle = spawn(async move {
        let gateway_classes = Api::<GatewayClass>::all(client);
        let watcher_config = Config::default();
        Controller::new(gateway_classes, watcher_config)
            .shutdown_on_signal()
            .run(
                async |gateway_class, state_tx| {
                    let parameters_ref = match &gateway_class.spec.parameters_ref {
                        Some(param_ref)
                            if param_ref.kind == GATEWAY_CLASS_PARAMETERS_CRD_KIND
                                && param_ref.group == GROUP =>
                        {
                            RefBuilder::default()
                                .name(&param_ref.name)
                                .namespace(None)
                                .build()
                                .ok()
                        }
                        _ => None,
                    };

                    let new_state = GatewayClassStateBuilder::default()
                        .name(gateway_class.name_any())
                        .parameter_ref(parameters_ref)
                        .build()
                        .ok();

                    state_tx.replace(new_state);

                    Ok(Action::requeue(Duration::from_secs(60)))
                },
                |_: Arc<GatewayClass>, _: &ControllerError, state_tx| {
                    state_tx.replace(None);
                    Action::requeue(Duration::from_secs(5))
                },
                Arc::new(state_tx),
            )
            .for_each(|_| ready(()))
            .await;
    });

    (join_handle, state_rx)
}
