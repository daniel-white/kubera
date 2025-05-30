mod computed_state;
mod controller;
use crate::api::v1alpha1::{GatewayClassParameters, GatewayParameters};
use crate::constants::MANAGED_BY_LABEL_QUERY;
use crate::spawn_controller;
use anyhow::Result;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{ConfigMap, Service};
use kube::runtime::watcher::Config;
use kube::Client;
use tokio::task::JoinSet;

pub async fn spawn_sources(join_set: &mut JoinSet<()>, client: &Client) -> Result<()> {
    let managed_by_selector = Config::default().labels(MANAGED_BY_LABEL_QUERY);

    let state_sources = computed_state::StateSources::new_builder()
        .gateway_classes(spawn_controller!(GatewayClass, join_set, client))
        .gateway_class_parameters(spawn_controller!(GatewayClassParameters, join_set, client))
        .gateways(spawn_controller!(Gateway, join_set, client))
        .gateway_parameters(spawn_controller!(GatewayParameters, join_set, client))
        .config_maps(spawn_controller!(
            ConfigMap,
            join_set,
            client,
            managed_by_selector.clone()
        ))
        .deployments(spawn_controller!(
            Deployment,
            join_set,
            client,
            managed_by_selector.clone()
        ))
        .services(spawn_controller!(
            Service,
            join_set,
            client,
            managed_by_selector.clone()
        ))
        .build()
        .expect("Failed to build StateSources");

    let computed_state = computed_state::spawn_controller(join_set, state_sources).await?;

    Ok(())
}
