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
        let yaml = r#"version: v1alpha1
hosts:
  - type: Exact
    value: api.example.com
  - value: .internal.example.net  # omitted 'type', defaults assumed elsewhere

http_routes:
  - host_headers:
      - value: api.example.com  # omitted 'type'
    rules:
      - matches:
          - path:
              type: Prefix
              value: /v1/users
        backends:
          - endpoints:
              - address: 10.0.1.10
                node: node-a
                zone: us-east-1a
            weight: 100  # omitted 'port'
  - host_headers:
      - type: Suffix
        value: .example.net
    rules:
      - matches:
          - method: POST
            path:
              type: Exact
              value: /submit
        backends:
          - endpoints:
              - address: 10.0.2.20
                zone: us-east-1b  # omitted 'node'
            port: 9090
            weight: 50
          - endpoints:
              - address: 10.0.2.21
            port: 9090
            weight: 50
  - host_headers:
      - value: admin.internal.example.net  # omitted 'type'
    rules:
      - matches:
          - path:
              type: RegularExpression
              value: ^/admin/.+$
            headers:
              - name: Authorization
                value: Bearer .+  # omitted 'type'
        backends:
          - endpoints:
              - address: 192.168.1.100
            port: 9443
            weight: 100
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
