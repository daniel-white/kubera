//! Common utilities for integration tests

use std::sync::Once;

static INIT: Once = Once::new();

/// Initialize test environment once
pub fn init_test_env() {
    INIT.call_once(|| {
        // Initialize crypto for tests
        kubera_core::crypto::init_crypto();

        // Set up test logging
        let _ = env_logger::builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Debug)
            .try_init();
    });
}

/// Create a test configuration for gateway
pub fn create_test_gateway_config() -> serde_yaml::Value {
    serde_yaml::from_str(
        r#"
apiVersion: gateway.networking.k8s.io/v1
kind: Gateway
metadata:
  name: test-gateway
  namespace: default
spec:
  gatewayClassName: kubera
  listeners:
  - name: http
    port: 80
    protocol: HTTP
  - name: https
    port: 443
    protocol: HTTPS
    tls:
      mode: Terminate
      certificateRefs:
      - name: test-cert
"#,
    )
    .unwrap()
}

/// Create a test HTTP route configuration
pub fn create_test_http_route() -> serde_yaml::Value {
    serde_yaml::from_str(
        r#"
apiVersion: gateway.networking.k8s.io/v1
kind: HTTPRoute
metadata:
  name: test-route
  namespace: default
spec:
  parentRefs:
  - name: test-gateway
  hostnames:
  - "example.com"
  rules:
  - matches:
    - path:
        type: PathPrefix
        value: "/api"
    backendRefs:
    - name: api-service
      port: 8080
"#,
    )
    .unwrap()
}
