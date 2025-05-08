use crate::api::constants::{GATEWAY_CLASS_PARAMETERS_CRD_KIND, GROUP};
use crate::controllers::state::{Ref, RefBuilder};
use derive_builder::Builder;
use derive_getters::Getters;
use futures::StreamExt;
use futures_signals::signal::Mutable;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use kube::{
    runtime::{controller::Action, watcher::Config, Controller}, Api, Client,
    ResourceExt,
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

pub struct GatewayClassController {
    client: Client,
    state: Arc<Mutable<Option<GatewayClassState>>>,
}

impl GatewayClassController {
    pub fn new(client: &Client) -> Self {
        Self {
            client: client.clone(),
            state: Arc::new(Mutable::new(None)),
        }
    }

    pub fn state(&self) -> Arc<Mutable<Option<GatewayClassState>>> {
        self.state.clone()
    }

    pub fn run(&self) -> JoinHandle<()> {
        let client = self.client.clone();
        let state = self.state.clone();
        spawn(async move {
            let gateway_classes = Api::<GatewayClass>::all(client);
            let watcher_config = Config::default();
            Controller::new(gateway_classes, watcher_config)
                .shutdown_on_signal()
                .run(
                    async |gateway_class, state| {
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
                        
                        state.set_neq(new_state);

                        Ok(Action::requeue(Duration::from_secs(60)))
                    },
                    |_: Arc<GatewayClass>, _: &ControllerError, _| {
                        Action::requeue(Duration::from_secs(5))
                    },
                    Arc::new(state),
                )
                .for_each(|_| ready(()))
                .await;
        })
    }
}
