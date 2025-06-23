use crate::objects::{ObjectRef, SyncObjectAction};
use crate::sync_objects;
use derive_builder::Builder;
use gtmpl_derive::Gtmpl;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::Client;
use tokio::sync::broadcast::channel;

const TEMPLATE: &str = include_str!("./templates/gateway_configmap.kubernetes-helm-yaml");

#[derive(Clone, Builder, Debug, Gtmpl)]
struct TemplateValues {
    gateway_name: String,
    config_yaml: String,
}

pub fn sync_gateway_configmaps(client: &Client) {
    let (tx, rx) = channel(1);

    sync_objects!(ConfigMap, client, rx, TemplateValues, TEMPLATE);
}
