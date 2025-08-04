//! Tests for configuration handling and validation

use crate::common::*;
use std::fs;
use tempfile::TempDir;
use test_log::test;

#[test]
async fn test_configuration_file_operations() {
    init_test_env();

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test-config.yaml");

    // Test writing configuration
    let config = create_test_gateway_config();
    let config_str = serde_yaml::to_string(&config).unwrap();
    fs::write(&config_path, &config_str).unwrap();

    // Test reading configuration back
    let read_content = fs::read_to_string(&config_path).unwrap();
    let parsed_config: serde_yaml::Value = serde_yaml::from_str(&read_content).unwrap();

    assert_eq!(parsed_config["metadata"]["name"], "test-gateway");
}

#[test]
async fn test_configuration_validation() {
    init_test_env();

    // Test valid hostnames
    let valid_hostnames = [
        "example.com",
        "api.example.com",
        "test-service",
        "localhost",
    ];

    for hostname in valid_hostnames {
        let h = kubera_core::net::Hostname::new(hostname);
        assert!(
            serde_valid::Validate::validate(&h).is_ok(),
            "Hostname '{}' should be valid",
            hostname
        );
    }

    // Test invalid hostnames
    let invalid_hostnames = [
        "",             // Empty
        ".",            // Just dot
        "example..com", // Double dots
        "-example.com", // Leading dash
        "example-.com", // Trailing dash
    ];

    for hostname in invalid_hostnames {
        let h = kubera_core::net::Hostname::new(hostname);
        assert!(
            serde_valid::Validate::validate(&h).is_err(),
            "Hostname '{}' should be invalid",
            hostname
        );
    }
}

#[test]
async fn test_port_validation() {
    init_test_env();

    // Test valid ports
    let valid_ports = [1u16, 80, 443, 8080, 65535];
    for port in valid_ports {
        let p = kubera_core::net::Port::new(port);
        assert!(serde_valid::Validate::validate(&p).is_ok());
    }
}

#[test]
async fn test_case_insensitive_operations() {
    init_test_env();

    let hostname1 = kubera_core::net::Hostname::new("Example.COM");
    let hostname2 = kubera_core::net::Hostname::new("example.com");

    // Should be equal despite different cases
    assert_eq!(hostname1, hostname2);

    // Should work in hash maps
    use std::collections::HashMap;
    let mut map = HashMap::new();
    map.insert(hostname1, "value");

    assert_eq!(map.get(&hostname2), Some(&"value"));
}

#[test]
async fn test_configuration_serialization_roundtrip() {
    init_test_env();

    // Test that we can serialize and deserialize configurations
    let original_config = create_test_gateway_config();

    // YAML roundtrip
    let yaml_str = serde_yaml::to_string(&original_config).unwrap();
    let from_yaml: serde_yaml::Value = serde_yaml::from_str(&yaml_str).unwrap();
    assert_eq!(original_config, from_yaml);

    // JSON roundtrip
    let json_str = serde_json::to_string(&original_config).unwrap();
    let from_json: serde_yaml::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(original_config, from_json);
}
