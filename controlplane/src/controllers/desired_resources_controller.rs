use crate::api::v1alpha1::{GatewayClassParameters, GatewayParameters};
use crate::constants::{MANAGED_BY_LABEL, MANAGED_BY_VALUE};
use crate::controllers::source_controller::SourceResources;
use crate::controllers::Ref;
use crate::select_continue;
use crate::sync::state::{channel, Receiver};
use derive_builder::Builder;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use getset::Getters;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{ConfigMap, Namespace, Service};
use thiserror::Error;
use tokio::task::JoinSet;

#[derive(Builder)]
pub struct SourceResourcesRecievers {
    gateway_classes: Receiver<SourceResources<GatewayClass>>,
    gateway_class_parameters: Receiver<SourceResources<GatewayClassParameters>>,
    gateways: Receiver<SourceResources<Gateway>>,
    gateway_parameters: Receiver<SourceResources<GatewayParameters>>,
    config_maps: Receiver<SourceResources<ConfigMap>>,
    deployments: Receiver<SourceResources<Deployment>>,
    services: Receiver<SourceResources<Service>>,
    namespaces: Receiver<SourceResources<Namespace>>,
}

impl SourceResourcesRecievers {
    pub fn new_builder() -> SourceResourcesRecieversBuilder {
        SourceResourcesRecieversBuilder::default()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DesiredResource<K: kube::Resource> {
    Create(K),
    Patch(K),
    Delete(Ref),
}

#[derive(Builder, Getters, Clone, Debug, PartialEq)]
pub struct NamespacedDesiredResources {
    #[getset(get = "pub")]
    namespace: String,

    #[getset(get = "pub")]
    config_maps: Vec<DesiredResource<ConfigMap>>,
    #[getset(get = "pub")]
    deployments: Vec<DesiredResource<Deployment>>,
    #[getset(get = "pub")]
    services: Vec<DesiredResource<Service>>,
}

impl NamespacedDesiredResources {
    pub fn new_builder() -> NamespacedDesiredResourcesBuilder {
        NamespacedDesiredResourcesBuilder::default()
    }
}

#[derive(Builder, Getters, Clone, Default, Debug, PartialEq)]
pub struct DesiredResources {
    #[getset(get = "pub")]
    namespaced: Vec<NamespacedDesiredResources>,
}

impl DesiredResources {
    pub fn new_builder() -> DesiredResourcesBuilder {
        DesiredResourcesBuilder::default()
    }
}

#[derive(Error, Debug)]
pub enum ControllerError {}

pub async fn spawn_controller(
    join_set: &mut JoinSet<()>,
    mut sources: SourceResourcesRecievers,
) -> Result<Receiver<Option<DesiredResources>>, ControllerError> {
    let (tx, rx) = channel::<Option<DesiredResources>>(None);

    join_set.spawn(async move {
        loop {
            let resources = NamespacedDesiredResources::new_builder()
                .namespace("default".to_string())
                .config_maps(vec![create_config_map()])
                .deployments(vec![])
                .services(vec![])
                .build()
                .expect("Failed to build resources");

            let desired_resources = DesiredResources::new_builder()
                .namespaced(vec![resources])
                .build()
                .expect("Failed to build resources");

            tx.replace(Some(desired_resources));

            select_continue!(
                sources.gateway_classes.changed(),
                sources.gateway_class_parameters.changed(),
                sources.gateways.changed(),
                sources.gateway_parameters.changed(),
                sources.config_maps.changed(),
                sources.deployments.changed(),
                sources.services.changed(),
                sources.namespaces.changed(),
            );
        }
    });

    Ok(rx)
}

fn create_config_map() -> DesiredResource<ConfigMap> {
    DesiredResource::Create(ConfigMap {
        metadata: kube::api::ObjectMeta {
            name: Some("example-configmap".to_string()),
            namespace: Some("default".to_string()),
            labels: Some(std::collections::BTreeMap::from([
                ("app".to_string(), "example".to_string()),
                (MANAGED_BY_LABEL.to_string(), MANAGED_BY_VALUE.to_string()),
            ])),
            ..Default::default()
        },
        data: Some(std::collections::BTreeMap::from([(
            "key".to_string(),
            "value".to_string(),
        )])),
        ..Default::default()
    })
}
