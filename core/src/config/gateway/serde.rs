use crate::config::gateway::types::GatewayConfiguration;
use serde_valid::Validate;
use serde_valid::validation::{Error, Errors};
use std::fmt::Debug;
use std::io::{Read, Write};
use thiserror::Error;
use tracing::{debug, instrument, warn};

#[derive(Debug, Error, Clone)]
pub enum ReadError {
    #[error("Failed to read configuration")]
    Error,

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(#[from] Errors<Error>),
}

#[instrument(skip(reader), level = "debug")]
pub fn read_configuration(reader: impl Read) -> Result<GatewayConfiguration, ReadError> {
    let configuration = serde_yaml::from_reader::<_, GatewayConfiguration>(reader)
        .inspect_err(|e| warn!("Failed to parse configuration: {}", e))
        .map_err(|_| ReadError::Error)?;

    configuration
        .validate()
        .inspect(|()| debug!("Read configuration is valid"))
        .inspect_err(|e| warn!("Invalid read configuration: {}", e))
        .map(|()| configuration)
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
pub fn write_configuration<W: Write>(
    config: &GatewayConfiguration,
    writer: &mut W,
) -> Result<(), WriteError> {
    config
        .validate()
        .map_err(WriteError::InvalidConfiguration)?;

    serde_yaml::to_writer(writer, &config).map_err(|_| WriteError::Error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::gateway::types::GatewayConfigurationVersion;
    use assertables::{assert_ok, assert_ok_eq};

    #[test]
    fn test_read_configuration() {
        let yaml = include_str!("tests/simple.yaml").as_bytes();

        let config = read_configuration(yaml);
        let version = config.map(|c| c.version());
        let version: Result<&GatewayConfigurationVersion, &ReadError> = version.as_ref();
        let expected: Result<&GatewayConfigurationVersion, &ReadError> =
            Ok(GatewayConfigurationVersion::V1Alpha1).as_ref();
        assert_ok_eq!(version, expected);
    }

    #[test]
    fn test_read_config1() {
        let yaml = include_str!("tests/config1.yaml").as_bytes();

        let config = read_configuration(yaml);
        let version = config.map(|c| c.version());
        let version: Result<&GatewayConfigurationVersion, &ReadError> = version.as_ref();
        let expected: Result<&GatewayConfigurationVersion, &ReadError> =
            Ok(GatewayConfigurationVersion::V1Alpha1).as_ref();
        assert_ok_eq!(version, expected);
    }

    #[test]
    fn round_trip_simple() {
        let yaml = include_str!("tests/simple.yaml").as_bytes();
        let config = assert_ok!(read_configuration(yaml));

        let mut buffer = Vec::new();
        assert_ok!(write_configuration(&config, &mut buffer));

        let round_trip_config = assert_ok!(read_configuration(buffer.as_slice()));
        assert_eq!(round_trip_config, config);
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
