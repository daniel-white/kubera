//! Tests for gateway functionality

use crate::common::*;
use test_log::test;

#[test]
async fn test_gateway_configuration_parsing() {
    init_test_env();

    let config = create_test_gateway_config();

    // Verify the configuration has expected structure
    assert_eq!(config["kind"], "Gateway");
    assert_eq!(config["metadata"]["name"], "test-gateway");
    assert_eq!(config["spec"]["listeners"].as_sequence().unwrap().len(), 2);
}

#[test]
async fn test_http_route_parsing() {
    init_test_env();

    let route = create_test_http_route();

    // Verify the route configuration
    assert_eq!(route["kind"], "HTTPRoute");
    assert_eq!(route["spec"]["hostnames"][0], "example.com");
    assert_eq!(
        route["spec"]["rules"][0]["matches"][0]["path"]["value"],
        "/api"
    );
}

#[test]
async fn test_gateway_listener_validation() {
    init_test_env();

    // Test valid port ranges
    let valid_ports = [80u16, 443, 8080, 3000];
    for port in valid_ports {
        let port_obj = kubera_core::net::Port::new(port);
        assert!(serde_valid::Validate::validate(&port_obj).is_ok());
    }
}

#[test]
async fn test_hostname_matching() {
    init_test_env();

    let hostname = kubera_core::net::Hostname::new("api.example.com");
    let domain_suffix = kubera_core::net::Hostname::new("example.com");
    let tld_suffix = kubera_core::net::Hostname::new("com");
    let wrong_suffix = kubera_core::net::Hostname::new("other.com");

    assert!(hostname.ends_with(&domain_suffix));
    assert!(hostname.ends_with(&tld_suffix));
    assert!(!hostname.ends_with(&wrong_suffix));
}
