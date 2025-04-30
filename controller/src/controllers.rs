use std::{
    future::ready,
    sync::{Arc, RwLock},
};

use futures::{StreamExt, TryStreamExt};
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::{
    Api, Client,
    runtime::{
        Controller, WatchStreamExt,
        controller::Action,
        watcher::{Config, watcher},
    },
};
use merge::Merge;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tokio::{
    join, spawn,
    sync::{
        Mutex,
        watch::{Receiver, channel},
    },
    time,
};

#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("Failed to get config map: {0}")]
    ConfigMapError(#[from] kube::Error),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ControllerConfig {
    #[serde(rename = "requeueInterval", with = "humantime_serde")]
    pub requeue_interval: Duration,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            requeue_interval: Duration::from_secs(60),
        }
    }
}

impl Merge for ControllerConfig {
    fn merge(&mut self, other: Self) {
        self.requeue_interval = other.requeue_interval;
    }
}

struct Context {
    client: Client,
    pub controller_config: Arc<RwLock<ControllerConfig>>,
}
impl Context {
    fn new(client: &Client) -> Self {
        Self {
            client: client.clone(),
            controller_config: Default::default(),
        }
    }
}

pub async fn run_controllers(client: &Client) -> anyhow::Result<()> {
    let context = Arc::new(Context::new(client));

    join!(watch_gateway_classes(context));

    Ok(())
}

async fn watch_gateway_classes(context: Arc<Context>) {
    let gateway_classes = Api::<GatewayClass>::all(context.client.clone());

    let (stop_watching_gateway_class_config_map_tx, stop_watching_gateway_class_config_map_rx) =
        channel(());
    let stop_watching_gateway_class_config_map_tx =
        Arc::new(Mutex::new(stop_watching_gateway_class_config_map_tx));
    let stop_watching_gateway_class_config_map_rx =
        Arc::new(Mutex::new(stop_watching_gateway_class_config_map_rx));

    let ctx = context.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(5)); // Run every 5 seconds

        loop {
            interval.tick().await; // Wait for the next tick
            let c = ctx.controller_config.read().unwrap();
            println!("Current requeue interval: {:?}", c.requeue_interval);
            // Perform your task here
        }
    });

    Controller::new(gateway_classes, Config::default())
        .run(
            |gateway_class, context| {
                let stop_watching_gateway_class_config_map_tx =
                    stop_watching_gateway_class_config_map_tx.clone();
                let stop_watching_gateway_class_config_map_rx =
                    stop_watching_gateway_class_config_map_rx.clone();
                async move {
                    println!("GatewayClass: {:?}", gateway_class.metadata.name);
                    if gateway_class.metadata.name.as_deref() == Some("kubera") {
                        stop_watching_gateway_class_config_map_tx
                            .lock()
                            .await
                            .send(())
                            .unwrap();
                        watch_gateway_class_config_map(
                            gateway_class,
                            context.clone(),
                            stop_watching_gateway_class_config_map_rx,
                        )
                    }
                    let config = context.controller_config.read().unwrap();
                    Ok(Action::requeue(config.requeue_interval))
                }
            },
            |_: Arc<GatewayClass>, _: &ControllerError, _| Action::requeue(Duration::from_secs(5)),
            context,
        )
        .for_each(|_| ready(()))
        .await;
}

fn watch_gateway_class_config_map(
    gateway_class: Arc<GatewayClass>,
    context: Arc<Context>,
    cancel_signal: Arc<Mutex<Receiver<()>>>,
) {
    spawn(async move {
        let mut cancel_signal = cancel_signal.lock().await;
        cancel_signal.mark_unchanged();

        let client = context.client.clone();
        let config_maps = Api::<ConfigMap>::all(client);
        let param_ref = match &gateway_class.spec.parameters_ref {
            Some(param_ref) if param_ref.kind == "ConfigMap" => param_ref,
            _ => return,
        };

        let watcher_future = watcher(
            config_maps,
            Config::default().fields(&format!("metadata.name={}", param_ref.name)),
        )
        .applied_objects()
        .try_for_each(|config_map| async {
            if let Some(data) = config_map.data {
                println!("ConfigMap data: {:?}", data);

                if let Some(config) = data.get("config") {
                    if let Ok(new_config) = serde_yaml::from_str::<ControllerConfig>(config) {
                        let mut controller_config = context.controller_config.write().unwrap();
                        controller_config.merge(new_config);
                    }
                }
            } else {
                println!("ConfigMap has no data");
            }
            Ok(())
        });

        tokio::select! {
            _ = watcher_future => {},
            _ = cancel_signal.changed() => {
                println!("Cancellation signal received, stopping watcher.");
            },
        }
    });
}
