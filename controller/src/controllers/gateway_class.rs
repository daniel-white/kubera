use crate::api::constants::{GATEWAY_CLASS_PARAMETERS_CRD_KIND, GROUP};
use crate::controllers::state::{Ref, RefBuilder};
use actix::WeakRecipient;
use actix::fut::wrap_future;
use actix::prelude::*;
use derive_builder::Builder;
use derive_getters::Getters;
use futures::StreamExt;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use kube::{
    Api, Client,
    runtime::{Controller, controller::Action, watcher::Config},
};
use std::sync::RwLock;
use std::{future::ready, sync::Arc, time::Duration};
use thiserror::Error;

#[derive(Builder, Getters, Message, Clone, Debug)]
#[rtype(result = "()")]
pub struct GatewayClassParametersRefChange {
    parameters_ref: Option<Ref>,
}

#[derive(Builder, Message, Debug)]
#[rtype(result = "()")]
pub struct SubscribeToGatewayClassParametersRefChange {
    recipient: WeakRecipient<GatewayClassParametersRefChange>,
}

pub struct GatewayClassController {
    client: Client,
    subscribers: Arc<RwLock<Vec<WeakRecipient<GatewayClassParametersRefChange>>>>,
}

impl GatewayClassController {
    pub fn new(client: Client) -> Self {
        GatewayClassController {
            client,
            subscribers: Arc::default(),
        }
    }
}

#[derive(Clone)]
struct ControllerContext {
    client: Client,
    subscribers: Arc<RwLock<Vec<WeakRecipient<GatewayClassParametersRefChange>>>>,
}

#[derive(Error, Debug)]
enum ControllerError {}

impl Actor for GatewayClassController {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        let client = self.client.clone();
        let subscribers = self.subscribers.clone();
        ctx.spawn(wrap_future(async move {
            let gateway_classes = Api::<GatewayClass>::all(client.clone());
            let watcher_config = Config::default();
            Controller::new(gateway_classes, watcher_config)
                .shutdown_on_signal()
                .run(
                    async |gateway_class, ctx| {
                        let parameters_ref = match &gateway_class.spec.parameters_ref {
                            Some(param_ref)
                                if param_ref.kind == GATEWAY_CLASS_PARAMETERS_CRD_KIND
                                    && param_ref.group == GROUP =>
                            {
                                Some(
                                    RefBuilder::default()
                                        .name(&param_ref.name)
                                        .namespace(None)
                                        .build()
                                        .expect("Failed to build Ref"),
                                )
                            }
                            _ => None,
                        };

                        let message = GatewayClassParametersRefChangeBuilder::default()
                            .parameters_ref(parameters_ref)
                            .build()
                            .expect("Failed to build GatewayClassParametersRefChange");

                        let subscribers = ctx.subscribers.read().unwrap();
                        for subscriber in subscribers.iter().filter_map(|s| s.upgrade()) {
                            subscriber.do_send(message.clone());
                        }

                        Ok(Action::requeue(Duration::from_secs(60)))
                    },
                    |_: Arc<GatewayClass>, _: &ControllerError, _| {
                        Action::requeue(Duration::from_secs(5))
                    },
                    Arc::new(ControllerContext {
                        client,
                        subscribers: subscribers.clone(),
                    }),
                )
                .for_each(|_| ready(()))
                .await;
        }));
    }
}

impl Supervised for GatewayClassController {}

impl Handler<SubscribeToGatewayClassParametersRefChange> for GatewayClassController {
    type Result = ();

    fn handle(
        &mut self,
        msg: SubscribeToGatewayClassParametersRefChange,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let mut clients = self.subscribers.write().unwrap();
        clients.push(msg.recipient)
    }
}
