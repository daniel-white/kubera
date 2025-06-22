use crate::objects::{ObjectRef, SyncObjectAction};
use crate::sync_objects;
use derive_builder::Builder;
use gtmpl_derive::Gtmpl;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::Client;
use tokio::sync::broadcast::channel;

const TEMPLATE: &str = include_str!("./templates/gateway_configmap.kubernetes-helm-yaml");

#[derive(Clone, Builder, Debug, Gtmpl)]
struct ConfigMapTemplateValues {
    gateway_name: String,
    config_yaml: String,
}

pub fn sync_gateway_configmaps(client: &Client) {
    let (tx, _) = channel(1);

    sync_objects!(ConfigMap, client, tx, ConfigMapTemplateValues, TEMPLATE);

    let or = ObjectRef::new_builder()
        .of_kind::<ConfigMap>()
        .name("gateway-configmap")
        .namespace(Some("default".to_string()))
        .build()
        .expect("Failed to build ObjectRef");

    let v = ConfigMapTemplateValuesBuilder::default()
        .gateway_name("example-gateway".to_string())
        .config_yaml("example: value".to_string())
        .build()
        .expect("Failed to build ConfigMapTemplateValues");

    tx.send(SyncObjectAction::Upsert((or, v)))
        .expect("Failed to send SyncObjectAction");
}
