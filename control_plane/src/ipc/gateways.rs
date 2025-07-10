use crate::kubernetes::objects::ObjectRef;
use dashmap::DashMap;
use dashmap::mapref::one::Ref;
use kubera_core::config::gateway::serde::{WriteError, write_configuration};
use kubera_core::config::gateway::types::GatewayConfiguration;
use std::io::{BufWriter, IntoInnerError};
use std::string::FromUtf8Error;
use std::sync::Arc;
use thiserror::Error;

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

#[derive(Debug, Error)]
pub enum GatewayConfigurationManagerInsertError {
    #[error("Failed to write configuration to buffer")]
    Write(#[from] WriteError),
    #[error("Failed to extract buffer from BufWriter")]
    Buffer(#[from] IntoInnerError<BufWriter<Vec<u8>>>),
    #[error("Failed to convert buffer to string")]
    Utf8(#[from] FromUtf8Error),
}

impl GatewayConfigurationManager {
    pub fn try_insert(
        &self,
        gateway_ref: ObjectRef,
        configuration: GatewayConfiguration,
    ) -> Result<(), GatewayConfigurationManagerInsertError> {
        let mut buf = BufWriter::new(Vec::new());
        write_configuration(&configuration, &mut buf)?;
        let buf = buf.into_inner()?;
        let yaml = String::from_utf8(buf)?;
        self.configurations.insert(gateway_ref, yaml);
        Ok(())
    }

    pub fn remove(&self, gateway_ref: &ObjectRef) -> bool {
        self.configurations.remove(gateway_ref).is_some()
    }
}
