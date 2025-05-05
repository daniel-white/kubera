use crate::api::constants::{GATEWAY_CLASS_PARAMETERS_CRD_KIND, GROUP};
use crate::controllers::gateway::watch_gateways;
use crate::controllers::gateway_class_parameters::watch_gateway_class_parameters;
use crate::controllers::state::{
    GatewayClassStateBuilder, RefBuilder, StateEvents,
};
use futures::StreamExt;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use kube::runtime::reflector::Lookup;
use kube::{
    runtime::{controller::Action, watcher::Config, Controller}, Api,
    Client,
};
use std::{future::ready, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::join;
use tokio::sync::mpsc::Sender;
use tokio::sync::watch::channel;

#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("Failed to get config map: {0}")]
    ConfigMapError(#[from] kube::Error),
}

pub async fn watch_gateway_class(client: &Client, state_events_tx: Sender<StateEvents>) {
    let (tx, rx) = channel(None);

    let parameters_watch = watch_gateway_class_parameters(&client, state_events_tx.clone(), rx.clone());
    let gateways_watch = watch_gateways(&client, state_events_tx.clone(), rx.clone());

    let gateway_classes = Api::<GatewayClass>::all(client.clone());
    let watcher_config = Config::default();
    let class_watch = Controller::new(gateway_classes, watcher_config)
        .shutdown_on_signal()
        .run(
            async |gateway_class, tx| {
                let mut state = GatewayClassStateBuilder::default();

                match gateway_class.name() {
                    Some(name) => {
                        state.name(name);
                    }
                    None => {
                        state.name("kubera");
                    }
                }

                if let Some(param_ref) = &gateway_class.spec.parameters_ref {
                    if param_ref.kind == GATEWAY_CLASS_PARAMETERS_CRD_KIND
                        && param_ref.group == GROUP
                    {
                        let parameters_ref = RefBuilder::default()
                            .name(&param_ref.name)
                            .build()
                            .expect("Failed to build Ref");
                        state.parameter_ref(Some(parameters_ref));
                    }
                }

                let config = state.build().expect("Failed to build GatewayClassState");
                let _ = tx.send(Some(config));

                Ok(Action::requeue(Duration::from_secs(60)))
            },
            |_: Arc<GatewayClass>, _: &ControllerError, _| Action::requeue(Duration::from_secs(5)),
            Arc::new(tx),
        )
        .for_each(|_| ready(()));

    join!(parameters_watch, gateways_watch, class_watch);
}
