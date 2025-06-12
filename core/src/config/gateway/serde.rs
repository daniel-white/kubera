use crate::config::gateway::types::GatewayConfiguration;
use serde_valid::validation::{Error, Errors};
use serde_valid::Validate;
use std::fmt::Debug;
use std::io::{Read, Write};
use thiserror::Error;
use tracing::{debug, instrument, warn};

#[derive(Debug, Error)]
pub enum ReadError {
    #[error("Failed to read configuration")]
    Error,

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(#[from] Errors<Error>),
}

#[instrument(skip(reader))]
pub fn read_configuration(reader: impl Read) -> Result<GatewayConfiguration, ReadError> {
    let configuration = serde_yaml::from_reader::<_, GatewayConfiguration>(reader)
        .inspect_err(|e| warn!("Failed to parse configuration: {}", e))
        .map_err(|_| ReadError::Error)?;

    configuration
        .validate()
        .inspect(|_| debug!("Read configuration is valid"))
        .inspect_err(|e| warn!("Invalid read configuration: {}", e))
        .map(|_| configuration)
        .map_err(ReadError::InvalidConfiguration)
}

#[derive(Debug, Error)]
pub enum WriteError {
    #[error("Failed to write configuration")]
    Error,

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(#[from] Errors<Error>),
}

#[instrument(skip(config, writer))]
pub fn write_configuration(
    config: &GatewayConfiguration,
    writer: impl Write,
) -> Result<(), WriteError> {
    config
        .validate()
        .map_err(WriteError::InvalidConfiguration)?;

    serde_yaml::to_writer(writer, config).map_err(|_| WriteError::Error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::gateway::types::GatewayConfigurationVersion;

    #[test]
    fn test_read_configuration() {
        let yaml = r#"
version: v1alpha1
hosts:
  - type: Exact
    value: gateway.example.com
  - type: Suffix
    value: ".service.local"

http_routes:
  - name: route-1
    namespace: production
    hosts:
      - type: Exact
        value: api.service.local
    rules:
      - matches:
          - method: POST
            path:
              type: Exact
              value: /submit
            headers:
              - name: Content-Type
                type: Exact
                value: application/json
              - name: X-Feature-Flag
                type: RegularExpression
                value: "^exp-.*"
            queryParams:
              - name: mode
                type: Exact
                value: async
        backend_refs:
          - name: backend-submit
            namespace: appspace
            port: 8443

  - name: route-2
    namespace: null
    hosts:
      - type: Suffix
        value: ".beta.example.com"
    rules:
      - matches:
          - method: GET
            path:
              type: Prefix
              value: /v2/data
            headers:
              - name: X-Debug
                type: Exact
                value: true
          - method: HEAD
            path:
              type: RegularExpression
              value: "^/health(/.+)?$"
            queryParams:
              - name: probe
                type: Exact
                value: readiness
        backend_refs:
          - name: backend-v2
            port: 9000
          - name: fallback-backend
            namespace: default
            port: null

service_backends:
  - name: backend-submit
    namespace: appspace
    addresses:
      - 192.168.10.1
      - 192.168.10.2

  - name: backend-v2
    namespace: default
    addresses:
      - 10.0.2.15
      - 10.0.2.16
      - 10.0.2.17

  - name: fallback-backend
    namespace: default
    addresses:
      - 127.0.0.1
        "#
        .as_bytes();

        let config = read_configuration(yaml);
        assert_eq!(
            *config.unwrap().version(),
            GatewayConfigurationVersion::V1Alpha1
        );
    }

    // #[test]
    // fn test_write_configuration() {
    //     let config = GatewayConfiguration {
    //         version: GatewayConfigurationVersion::V1Alpha1,
    //         hosts
    //     };
    //
    //     let mut buffer = Vec::new();
    //     write_configuration(&config, &mut buffer).unwrap();
    //     assert!(!buffer.is_empty());
    // }
}
