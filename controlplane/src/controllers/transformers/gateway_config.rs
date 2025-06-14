use crate::constants::{
    CONFIGMAP_ROLE_GATEWAY_CONFIG, CONFIGMAP_ROLE_LABEL, MANAGED_BY_LABEL, MANAGED_BY_VALUE,
};
use crate::controllers::objects::{ObjectRef, ObjectState, Objects};
use anyhow::Result;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::api::{Patch, PatchParams, PostParams};
use kube::Client;
use kubera_core::config::gateway::serde::write_configuration;
use kubera_core::config::gateway::types::http::router::{HttpRoute, HttpRouteBuilder};
use kubera_core::config::gateway::types::{
    GatewayConfiguration, GatewayConfigurationBuilder, GatewayConfigurationVersion,
};
use kubera_core::select_continue;
use kubera_core::sync::signal::{channel, Receiver};
use std::collections::{BTreeMap, HashMap};
use std::io::BufWriter;
use tokio::task::JoinSet;
use tracing::warn;

pub fn generate_gateway_configuration(
    join_set: &mut JoinSet<()>,
    gateways: &Receiver<Objects<Gateway>>,
) -> Receiver<HashMap<ObjectRef, GatewayConfiguration>> {
    let (tx, rx) = channel(HashMap::default());

    let mut gateways = gateways.clone();

    join_set.spawn(async move {
        loop {
            let current_gateways = gateways.current();
            let configs: HashMap<_, _> = current_gateways
                .iter()
                .map(|(gateway_ref, _, gateway)| {
                    warn!("Generating configuration for gateway: {}", gateway_ref);
                    let c = GatewayConfigurationBuilder::default()
                        .version(GatewayConfigurationVersion::V1Alpha1)
                        .hosts(vec![])
                        .http_routes(vec![
                            HttpRouteBuilder::default()
                                .host_header_matches(vec![])
                                .rules(vec![])
                                .build()
                                .unwrap(),
                        ])
                        .build()
                        .unwrap();
                    (gateway_ref.clone(), c)
                })
                .collect();

            tx.replace(configs);

            select_continue!(gateways.changed());
        }
    });

    rx
}

pub fn sync_gateway_configuration(
    join_set: &mut JoinSet<()>,
    client: &Client,
    config_maps: &Receiver<Objects<ConfigMap>>,
    gateway_configurations: &Receiver<HashMap<ObjectRef, GatewayConfiguration>>,
) {
    let mut config_maps = config_maps.clone();
    let mut gateway_configuration = gateway_configurations.clone();
    let client = client.clone();

    join_set.spawn(async move {
        loop {
            let current_config_maps = config_maps.current();
            let current_gateway_configs = gateway_configuration.current();
            for (gateway_ref, gateway_config) in current_gateway_configs.iter() {
                if let Ok((config_map_ref, config_map)) =
                    map_gateway_configuration_to_config_map(gateway_ref, gateway_config)
                {
                    let config_maps_api = kube::Api::<ConfigMap>::namespaced(
                        client.clone(),
                        config_map_ref.namespace().clone().as_deref().unwrap(),
                    );

                    match current_config_maps.as_ref().get_by_ref(&config_map_ref) {
                        Some(ObjectState::Active(_)) => {
                            config_maps_api
                                .patch(
                                    config_map_ref.name(),
                                    &PatchParams::default(),
                                    &Patch::Strategic(config_map),
                                )
                                .await
                                .inspect_err(|e| {
                                    warn!(
                                        "Failed to update ConfigMap {}: {}",
                                        config_map_ref.name(),
                                        e
                                    );
                                });
                        }
                        _ => {
                            config_maps_api
                                .create(&PostParams::default(), &config_map)
                                .await
                                .inspect_err(|e| {
                                    warn!(
                                        "Failed to create ConfigMap {}: {}",
                                        config_map_ref.name(),
                                        e
                                    );
                                });
                        }
                    }
                }
            }
            select_continue!(config_maps.changed(), gateway_configuration.changed());
        }
    });
}

fn map_gateway_configuration_to_config_map(
    gateway_ref: &ObjectRef,
    gateway_config: &GatewayConfiguration,
) -> Result<(ObjectRef, ConfigMap)> {
    let config_map_name = format!("{}-gateway-config", gateway_ref.name());
    let config_map_ref = ObjectRef::new_builder()
        .of_kind::<ConfigMap>()
        .namespace(gateway_ref.namespace().clone())
        .name(&config_map_name)
        .build()
        .expect("Failed to build ConfigMap reference");

    let mut buf = BufWriter::new(Vec::new());
    write_configuration(gateway_config, &mut buf)?;
    let file_content = String::from_utf8(buf.into_inner()?)?;

    let config_map = ConfigMap {
        metadata: kube::api::ObjectMeta {
            name: Some(config_map_name),
            namespace: gateway_ref.namespace().clone(),
            labels: Some(BTreeMap::from([
                (MANAGED_BY_LABEL.to_string(), MANAGED_BY_VALUE.to_string()),
                (
                    CONFIGMAP_ROLE_LABEL.to_string(),
                    CONFIGMAP_ROLE_GATEWAY_CONFIG.to_string(),
                ),
            ])),
            ..Default::default()
        },
        data: Some(BTreeMap::from([("config.yaml".to_string(), file_content)])),
        ..Default::default()
    };

    warn!(
        "Hello world! Mapping gateway configuration to ConfigMap: {:?}",
        config_map
    );

    Ok((config_map_ref, config_map))
}
