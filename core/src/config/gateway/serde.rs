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
    value: edge.example.com
  - type: Suffix
    value: ".cluster.local"

http_routes:
  - name: main-api
    namespace: production
    hosts:
      - type: Exact
        value: api.edge.example.com
    rules:
      - matches:
          - method: GET
            path:
              type: Prefix
              value: /v1/
            headers:
              - name: X-Env
                type: Exact
                value: prod
              - name: X-Trace-ID
                type: RegularExpression
                value: "^trace-[a-z0-9]+$"
            queryParams:
              - name: verbose
                type: Exact
                value: "true"
        backend_refs:
          - name: users-backend
            namespace: core-services
            port: 8080
          - name: analytics-backend
            namespace: null
            port: 9090

  - name: canary-check
    namespace: null
    hosts:
      - type: Suffix
        value: ".internal.example.net"
    rules:
      - matches:
          - method: POST
            path:
              type: Exact
              value: /experiment
        backend_refs:
          - name: canary-backend
            port: 8000

service_backends:
  - name: users-backend
    namespace: core-services
    backends:
      - addresses:
          - 10.0.0.5
          - 10.0.0.6
        node: node-a-1
        zone: zone-us-east
      - addresses:
          - 10.0.1.5
        node: node-a-2
        zone: zone-us-west

  - name: analytics-backend
    namespace: null
    backends:
      - addresses:
          - 192.168.10.100
        node: analytics-node-1
        zone: zone-eu-central

  - name: canary-backend
    namespace: staging
    backends:
      - addresses:
          - 10.42.0.77
        node: null
        zone: null
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
