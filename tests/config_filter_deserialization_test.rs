use kubera_core::config::gateway::types::http::filters::{
    HTTPHeader, HTTPRouteFilter, HTTPRouteFilterType, RequestHeaderModifier,
};
use kubera_core::config::gateway::GatewayConfig;
use serde_yaml;

#[cfg(test)]
mod config_filter_tests {
    #[test]
    fn test_config_with_filters_deserialization() {
        // This is what a Kubera config.yaml should look like with filters
        let config_yaml = r#"
version: v1alpha1
ipc:
  endpoint: "127.0.0.1:9090"
listeners:
  - name: "http-listener"
    port: 8080
    protocol: "HTTP"
http_routes:
  - rules:
      - unique_id: "test-route-with-filters"
        matches:
          - path:
              type: "PathPrefix"
              value: "/api"
        filters:
          - type: "RequestHeaderModifier"
            request_header_modifier:
              set:
                - name: "X-Gateway"
                  value: "kubera-gateway"
                - name: "X-Route-Name"
                  value: "test-route"
              add:
                - name: "X-Request-ID"
                  value: "generated-uuid-12345"
              remove:
                - "X-Debug-Info"
                - "X-Internal-Header"
        backends:
          - name: "test-service"
            namespace: "default"
            port: 80
            weight: 100
            endpoints:
              - address: "10.0.1.10"
                node: "worker-1"
                zone: "us-west-2a"
client_addrs:
  source: "Proxies"
  proxies:
    trusted_networks:
      - "10.0.0.0/8"
error_responses:
  kind: "ProblemDetail"
"#;

        // Try to deserialize the config
        let config_result: Result<GatewayConfig, _> = serde_yaml::from_str(config_yaml);
        match config_result {
            Ok(config) => {
                println!("Successfully deserialized config: {:#?}", config);

                // Check that filters were properly deserialized
                assert!(!config.http_routes().is_empty());
                let route = &config.http_routes()[0];
                assert!(!route.rules().is_empty());
                let rule = &route.rules()[0];
                assert!(!rule.filters().is_empty());

                let filter = &rule.filters()[0];
                assert_eq!(
                    filter.filter_type,
                    HTTPRouteFilterType::RequestHeaderModifier
                );
                assert!(filter.request_header_modifier.is_some());

                let modifier = filter.request_header_modifier.as_ref().unwrap();
                assert!(modifier.set().is_some());
                assert!(modifier.add().is_some());
                assert!(modifier.remove().is_some());

                println!("Filter deserialization test passed!");
            }
            Err(e) => {
                println!("Failed to deserialize config with filters: {}", e);
                panic!("Config deserialization failed: {}", e);
            }
        }
    }

    #[test]
    fn test_existing_config_without_filters() {
        // Test the existing simple.yaml config to see if it deserializes correctly
        let simple_config = r#"
version: v1alpha1
ipc:
  endpoint: 10.244.0.56:8080
listeners:
  - name: http
    host: null
    port: 80
    protocol: HTTP
http_routes:
  - rules:
      - unique_id: dc74f547-fa64-4b83-b600-c1c9895b2ad2:aec2951c-ab8d-4d8e-a4cd-7b805a49734e:0
        matches:
          - method: GET
        backends:
          - weight: 1
            port: 80
            name: echo
            namespace: default
            endpoints:
              - node: minikube
                address: 10.244.0.67
"#;

        let config: GatewayConfig = serde_yaml::from_str(simple_config).unwrap();
        println!("Existing config deserialized: {:#?}", config);

        // Check that filters are empty (as expected)
        let route = &config.http_routes()[0];
        let rule = &route.rules()[0];
        assert!(rule.filters().is_empty());

        println!("Existing config has no filters as expected");
    }

    #[test]
    fn test_filter_structure_directly() {
        // Test just the filter structure itself
        let filter_yaml = r#"
type: "RequestHeaderModifier"
request_header_modifier:
  set:
    - name: "X-Gateway"
      value: "kubera-gateway"
  add:
    - name: "X-Request-ID"
      value: "test-id"
  remove:
    - "X-Debug-Info"
"#;

        let filter_result: Result<HTTPRouteFilter, _> = serde_yaml::from_str(filter_yaml);
        match filter_result {
            Ok(filter) => {
                println!("Successfully deserialized filter: {:#?}", filter);
                assert_eq!(
                    filter.filter_type,
                    HTTPRouteFilterType::RequestHeaderModifier
                );
                assert!(filter.request_header_modifier.is_some());
            }
            Err(e) => {
                println!("Failed to deserialize filter: {}", e);
                panic!("Filter deserialization failed: {}", e);
            }
        }
    }
}
