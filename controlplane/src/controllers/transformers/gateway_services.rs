use crate::controllers::transformers::Backend;
use crate::ipc::IpcServices;
use crate::objects::{ObjectRef, ObjectState, Objects};
use anyhow::Result;
use gateway_api::apis::standard::gateways::Gateway;
use gateway_api::apis::standard::httproutes::{
    HTTPRoute, HTTPRouteRulesMatchesHeadersType, HTTPRouteRulesMatchesMethod,
    HTTPRouteRulesMatchesPathType, HTTPRouteRulesMatchesQueryParamsType,
};
use gtmpl_derive::Gtmpl;
use k8s_openapi::api::core::v1::{ConfigMap, Service};
use kube::api::{Patch, PatchParams, PostParams};
use kube::Client;
use kubera_api::constants::{
    CONFIGMAP_ROLE_GATEWAY_CONFIG, CONFIGMAP_ROLE_LABEL, MANAGED_BY_LABEL, MANAGED_BY_VALUE,
};
use kubera_core::config::gateway::serde::write_configuration;
use kubera_core::config::gateway::types::http::router::HttpMethodMatch;
use kubera_core::config::gateway::types::{GatewayConfiguration, GatewayConfigurationBuilder};
use kubera_core::continue_on;
use kubera_core::ipc::{Event, GatewayEvent};
use kubera_core::net::Hostname;
use kubera_core::sync::signal::{channel, Receiver};
use std::collections::hash_map::{Entry, HashMap};
use std::collections::BTreeMap;
use std::io::BufWriter;
use std::sync::Arc;
use tokio::spawn;
use tracing::warn;

const SERVICE_TEMPLATE: &str =
    include_str!("../../../templates/gateway_service.kubernetes-helm-yaml");

#[derive(Gtmpl)]
struct Foo {
    namespace: String,
    name: String,
}

pub fn generate_gateway_services(
    gateways: &Receiver<Objects<Gateway>>,
    ipc_services: Arc<IpcServices>,
) -> Receiver<HashMap<ObjectRef, Service>> {
    let (tx, rx) = channel(HashMap::default());

    let mut gateways = gateways.clone();

    spawn(async move {
        loop {
            let current_gateways = gateways.current();

            for (gateway_ref, _, gateway) in current_gateways.iter() {
                match gateway {
                    ObjectState::Active(gateway) => {
                        let output = gtmpl::template(
                            SERVICE_TEMPLATE,
                            Foo {
                                namespace: gateway_ref
                                    .namespace()
                                    .clone()
                                    .unwrap_or("default".to_string()),
                                name: gateway_ref.name().to_string(),
                            },
                        );
                        let svc = serde_yaml::from_str::<Service>(output.unwrap().as_str());

                        warn!("Generating service for Gateway: {:?}", svc);
                    }
                    ObjectState::Deleted(_) => {}
                }
            }

            continue_on!(gateways.changed());
        }
    });

    rx
}
