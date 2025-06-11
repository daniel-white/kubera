use crate::api::v1alpha1::{GatewayClassParameters, GatewayParameters};
use crate::constants::{MANAGED_BY_LABEL, MANAGED_BY_VALUE};
use crate::controllers::resources::{Ref, Resources};
use derive_builder::Builder;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::{Gateway, GatewayStatus};
use getset::Getters;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{ConfigMap, Namespace, Service};
use kube::api::Patch;
use kubera_core::select_continue;
use kubera_core::sync::signal::{Receiver, channel};
use serde::Serialize;
use std::collections::HashMap;
use thiserror::Error;
use tokio::task::JoinSet;

#[derive(Builder)]
pub struct SourceResourcesReceivers {
    gateway_classes: Receiver<Resources<GatewayClass>>,
    gateway_class_parameters: Receiver<Resources<GatewayClassParameters>>,
    gateways: Receiver<Resources<Gateway>>,
    gateway_parameters: Receiver<Resources<GatewayParameters>>,
    config_maps: Receiver<Resources<ConfigMap>>,
    deployments: Receiver<Resources<Deployment>>,
    services: Receiver<Resources<Service>>,
    namespaces: Receiver<Resources<Namespace>>,
}

impl SourceResourcesReceivers {
    pub fn new_builder() -> SourceResourcesReceiversBuilder {
        SourceResourcesReceiversBuilder::default()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DesiredResource<K: kube::Resource + Serialize> {
    Create(K),
    Patch(String, Patch<K>),
    Delete(Ref),
}

#[derive(Builder, Getters, Clone, Debug, PartialEq)]
pub struct DesiredGateway {
    #[getset(get = "pub")]
    gateway_ref: Ref,
    #[getset(get = "pub")]
    status: GatewayStatus,
    #[getset(get = "pub")]
    config_map: DesiredResource<ConfigMap>,
    #[getset(get = "pub")]
    deployment: DesiredResource<Deployment>,
    #[getset(get = "pub")]
    service: DesiredResource<Service>,
}

impl DesiredGateway {
    pub fn new_builder() -> DesiredGatewayBuilder {
        DesiredGatewayBuilder::default()
    }
}

#[derive(Builder, Getters, Clone, Default, Debug, PartialEq)]
pub struct DesiredResources {
    #[getset(get = "pub")]
    gateways: HashMap<Ref, DesiredGateway>,
    #[getset(get = "pub")]
    gateway_classes: HashMap<Ref, GatewayClass>,
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
    mut sources: SourceResourcesReceivers,
) -> Result<Receiver<Option<DesiredResources>>, ControllerError> {
    let (tx, rx) = channel::<Option<DesiredResources>>(None);

    join_set.spawn(async move {
        loop {
            // let gateway_class_parameters = sources.gateway_class_parameters.current();
            // let gateway_class_parameters = gateway_class_parameters.resources();
            // let gateway_classes: Vec<_> = sources
            //     .gateway_classes
            //     .current()
            //     .resources()
            //     .into_iter()
            //     .map(|(ref_, state)| match state {
            //         Active(gateway_class)
            //             if gateway_class.spec.controller_name == GATEWAY_CLASS_CONTROLLER_NAME =>
            //         {
            //             let process_result = processors::gateway_class::process(gateway_class, gateway_class_parameters);
            //         }
            //         _ => None,
            //     })
            //     .collect();
            //
            // let gateway_classes: HashMap<Ref, GatewayClass> = gateway_classes
            //     .iter()
            //     .map(|(ref_, gateway_class, parameters)| {
            //         let mut gateway_class = gateway_class.clone();
            //         gateway_class.status = Some(GatewayClassStatus {
            //             conditions: Some(vec![Condition {
            //                 type_: "Accepted".to_string(),
            //                 status: "True".to_string(),
            //                 last_transition_time: Time(DateTime::<Utc>::from(SystemTime::now())),
            //                 message: "Hello, World!".to_string(),
            //                 reason: "ExampleReason".to_string(),
            //                 observed_generation: None,
            //             }]),
            //         });
            //         (ref_.clone(), gateway_class)
            //     })
            //     .collect();

            let desired_resources = DesiredResources::new_builder()
                .gateways(Default::default())
                .gateway_classes(Default::default())
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
    DesiredResource::Patch(
        "example-configmap".to_string(),
        Patch::Strategic(ConfigMap {
            metadata: kube::api::ObjectMeta {
                name: Some("example-configmap".to_string()),
                namespace: Some("default".to_string()),
                labels: Some(std::collections::BTreeMap::from([
                    ("app".to_string(), "example".to_string()),
                    (MANAGED_BY_LABEL.to_string(), MANAGED_BY_VALUE.to_string()),
                ])),
                ..Default::default()
            },
            data: Some(std::collections::BTreeMap::from([
                ("key".to_string(), "valueupdatesat".to_string()),
                ("hello".to_string(), "value2".to_string()),
            ])),
            ..Default::default()
        }),
    )
}
