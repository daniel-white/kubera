use crate::api::constants::{GATEWAY_CLASS_PARAMETERS_CRD_KIND, GROUP};
use crate::controllers::state::{Ref, RefBuilder};
use futures::StreamExt;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use kube::{
    runtime::{controller::Action, watcher::Config, Controller}, Api,
    Client,
};
use ractor::{Actor, ActorProcessingErr, ActorRef};
use std::sync::RwLock;
use std::{future::ready, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::spawn;

#[derive(Clone)]
pub struct GatewayClassControllerState {
    subscribers: Arc<RwLock<Vec<ActorRef<GatewayClassControllerMessage>>>>,
}

#[derive(Clone)]
pub enum GatewayClassControllerMessage {
    ParametersRefChange(Option<Ref>),
    Subscribe(ActorRef<GatewayClassControllerMessage>),
}

pub struct GatewayClassController {
    client: Client,
}

impl GatewayClassController {
    pub fn new(client: &Client) -> Self {
        Self { client: client.clone() }
    }
}

#[derive(Clone)]
struct ControllerContext {
    client: Client,
    subscribers: Arc<RwLock<Vec<ActorRef<GatewayClassControllerMessage>>>>,
}

#[derive(Error, Debug)]
enum ControllerError {}

impl Actor for GatewayClassController {
    type State = GatewayClassControllerState;
    type Msg = GatewayClassControllerMessage;
    type Arguments = ();

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let client = self.client.clone();
        let subscribers: Arc<RwLock<Vec<ActorRef<Self::Msg>>>> = Arc::default();
        let cloned_subscribers = subscribers.clone();

        let result = spawn(async move {
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
                                        .build()
                                        .expect("Failed to build Ref"),
                                )
                            }
                            _ => None,
                        };

                        let message = Self::Msg::ParametersRefChange(parameters_ref);

                        let subscribers = ctx.subscribers.read().unwrap();
                        for subscriber in subscribers.iter() {
                            subscriber.send_message(message.clone()).unwrap();
                        }

                        Ok(Action::requeue(Duration::from_secs(60)))
                    },
                    |_: Arc<GatewayClass>, _: &ControllerError, _| {
                        Action::requeue(Duration::from_secs(5))
                    },
                    Arc::new(ControllerContext {
                        client,
                        subscribers: cloned_subscribers,
                    }),
                )
                .for_each(|_| ready(()))
                .await;
        })
        .await;
        
        Ok(GatewayClassControllerState {
            subscribers,
        })
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            GatewayClassControllerMessage::ParametersRefChange(_) => Ok(()),
            
            GatewayClassControllerMessage::Subscribe(actor) => {
                let mut subscribers = state.subscribers.write().unwrap();
                subscribers.push(actor);
                Ok(())
            }
        }
    }
}