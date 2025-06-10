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
  - hostnames:
      - value: ".example.com"
        type: Suffix
      - value: "api.example.com"
    httpRoutes:
      - name: "get-users-route"
        matches:
          - methods:
            - GET
            paths:
            - type: Prefix
              value: "/users"
            headers:
              - name: "X-Requested-With"
                type: Exact
                value: "XMLHttpRequest"
              - name: "User-Agent"
                type: RegularExpression
                value: ".*Chrome.*"
            queryParams:
              - name: "active"
                type: Exact
                value: "true"
              - name: "sort"
                type: RegularExpression
                value: "asc|desc"
        backends:
          - name: "user-service"
            namespace: "default"
            kind: "Service"
            group: "core"
            port: 8080
      - name: "post-users-route"
        matches:
          - methods:
            - POST
            paths:
            - type: Exact
              value: "/users/create"
            headers:
              - name: "Content-Type"
                type: Exact
                value: "application/json"
        backends:
          - name: "user-service"
            namespace: "default"
            kind: "Service"
            group: "core"
            port: 8081

  - hostnames:
      - value: "admin.example.com"
    httpRoutes:
      - name: "admin-dashboard"
        matches:
          - methods:
            - GET
            paths:
            - type: Exact
              value: "/dashboard"
        backends:
          - name: "admin-ui"
            namespace: "admin"
            kind: "Deployment"
            group: "apps"
            port: 443
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
