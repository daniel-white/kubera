use crate::kubernetes::objects::ObjectRef;
use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use kubera_core::config::gateway::serde::write_configuration;
use kubera_core::config::gateway::types::GatewayConfiguration;
use std::io::BufWriter;
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
        let mut buf = BufWriter::new(Vec::new());
        if write_configuration(configuration, &mut buf).is_ok() {
            let yaml = String::from_utf8(buf.into_inner().unwrap()).unwrap();
            self.configurations.insert(gateway_ref, yaml);
        }
    }

    pub fn remove(&self, gateway_ref: &ObjectRef) {
        self.configurations.remove(gateway_ref);
    }
}
