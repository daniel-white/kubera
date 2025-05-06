use crate::api::v1alpha1::GatewayClassParameters;
use crate::controllers::gateway_class::GatewayClassParametersRefChange;
use crate::controllers::state::Ref;
use actix::prelude::*;
use actix::WeakRecipient;
use derive_builder::Builder;
use futures::StreamExt;
use kube::runtime::controller::Action;
use kube::runtime::watcher::Config;
use kube::runtime::Controller;
use kube::{Api, Client};
use std::future::ready;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use thiserror::Error;

#[derive(Builder, Message, Clone, Debug)]
#[rtype(result = "()")]
pub struct GatewayClassParametersChange {}

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct SubscribeToGatewayClassParametersChange(pub WeakRecipient<GatewayClassParametersChange>);

pub struct GatewayClassParametersController {
    client: Client,
    parameters_ref: Option<Ref>,
    subscribers: Arc<RwLock<Vec<WeakRecipient<GatewayClassParametersChange>>>>,
}

impl GatewayClassParametersController {
    pub fn new(client: Client) -> Self {
        GatewayClassParametersController {
            client,
            parameters_ref: None,
            subscribers: Arc::default(),
        }
    }
}

#[derive(Clone)]
struct ControllerContext {
    client: Client,
    subscribers: Arc<RwLock<Vec<WeakRecipient<GatewayClassParametersChange>>>>,
}

#[derive(Error, Debug)]
enum ControllerError {}

impl Actor for GatewayClassParametersController {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        let client = self.client.clone();
        let subscribers = self.subscribers.clone();
        let fut = Box::pin(async move {
            let parameters = Api::<GatewayClassParameters>::all(client.clone());
            let watcher_config = Config::default();
            Controller::new(parameters, watcher_config)
                .run(
                    async |parameters, ctx| Ok(Action::requeue(Duration::from_secs(60))),
                    |_, _: &ControllerError, _| Action::requeue(Duration::from_secs(5)),
                    Arc::new(ControllerContext {
                        client,
                        subscribers: subscribers.clone(),
                    }),
                )
                .for_each(|_| ready(()))
                .await
        });

        let actor_fut = fut.into_actor(self);

        ctx.spawn(actor_fut);
    }
}

impl Supervised for GatewayClassParametersController {
    fn restarting(&mut self, ctx: &mut <Self>::Context) {
        println!("restarting");
    }
}

impl Handler<GatewayClassParametersRefChange> for GatewayClassParametersController {
    type Result = ();
    fn handle(
        &mut self,
        msg: GatewayClassParametersRefChange,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        self.parameters_ref = msg.parameters_ref().clone();
        ctx.stop()
    }
}
