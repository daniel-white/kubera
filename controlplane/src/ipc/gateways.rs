use crate::objects::ObjectRef;
use dashmap::DashMap;
use dashmap::mapref::one::Ref;
use kubera_core::config::gateway::types::GatewayConfiguration;
use std::sync::Arc;

pub fn create_gateway_configuration_services()
-> (GatewayConfigurationReader, GatewayConfigurationManager) {
    let configurations = Arc::new(DashMap::new());
    (
        GatewayConfigurationReader {
            configurations: configurations.clone(),
        },
        GatewayConfigurationManager { configurations },
    )
}

#[derive(Debug, Clone)]
pub struct GatewayConfigurationReader {
    configurations: Arc<DashMap<ObjectRef, String>>,
}

impl GatewayConfigurationReader {
    pub fn exists(&self, gateway_ref: &ObjectRef) -> bool {
        self.configurations.contains_key(gateway_ref)
    }

    pub fn get_configuration_yaml(
        &self,
        gateway_ref: &ObjectRef,
    ) -> Option<Ref<ObjectRef, String>> {
        self.configurations.get(gateway_ref)
    }
}

#[derive(Debug, Clone)]
pub struct GatewayConfigurationManager {
    configurations: Arc<DashMap<ObjectRef, String>>,
}

impl GatewayConfigurationManager {
    pub fn insert(&self, gateway_ref: ObjectRef, configuration: &GatewayConfiguration) {
        let yaml = serde_yaml::to_string(configuration)
            .expect("Failed to serialize GatewayConfiguration to YAML");
        self.configurations.insert(gateway_ref, yaml);
    }

    pub fn remove(&self, gateway_ref: &ObjectRef) {
        self.configurations.remove(gateway_ref);
    }
}
