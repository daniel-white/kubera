mod desired_resources;
mod resulting_resources_controller;
mod source_controller;

use crate::api::v1alpha1::{GatewayClassParameters, GatewayParameters};
use crate::constants::MANAGED_BY_LABEL_QUERY;
use crate::spawn_controller;
use anyhow::Result;
use derive_builder::Builder;
use desired_resources::controller as desired_resources_controller;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use getset::Getters;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{ConfigMap, Namespace, Service};
use kube::Client;
use kube::runtime::watcher::Config;
use tokio::task::JoinSet;

pub async fn run() -> Result<()> {
    let mut join_set = JoinSet::new();
    let client = Client::try_default().await?;

    let managed_by_selector = Config::default().labels(MANAGED_BY_LABEL_QUERY);

    let sources = desired_resources_controller::SourceResourcesReceivers::new_builder()
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
        .namespaces(spawn_controller!(Namespace, join_set, client))
        .build()
        .expect("Failed to build sources");

    let desired_resources =
        desired_resources_controller::spawn_controller(&mut join_set, sources).await?;

    resulting_resources_controller::spawn_controller(&mut join_set, &client, desired_resources)
        .await?;

    join_set.join_all().await;

    Ok(())
}

#[derive(Builder, Getters, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[builder(setter(into))]
pub struct Ref {
    #[getset(get = "pub")]
    namespace: Option<String>,

    #[getset(get = "pub")]
    name: String,
}

impl Ref {
    pub fn new_builder() -> RefBuilder {
        RefBuilder::default()
    }
}
