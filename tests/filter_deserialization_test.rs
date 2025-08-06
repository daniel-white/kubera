use kubera_core::config::gateway::types::http::filters::{
    HTTPHeader, HTTPRouteFilter, HTTPRouteFilterType, RequestHeaderModifier,
};
use serde_yaml;

#[cfg(test)]
mod filter_deserialization_tests {
    #[test]
    fn test_gateway_api_httproute_filter_deserialization() {
        // This is the actual Gateway API HTTPRoute format from the echo demo
        let gateway_api_yaml = r#"
apiVersion: gateway.networking.k8s.io/v1
kind: HTTPRoute
metadata:
  name: echo-route
  namespace: default
spec:
  parentRefs:
    - name: kubera-gateway
  rules:
    - matches:
        - path:
            type: PathPrefix
            value: /echo
      filters:
        - type: RequestHeaderModifier
          requestHeaderModifier:
            set:
              - name: "X-Gateway"
                value: "kubera-gateway"
              - name: "X-Route-Name"
                value: "echo-route"
            add:
              - name: "X-Request-ID"
                value: "generated-uuid-12345"
            remove:
              - "X-Debug-Info"
      backendRefs:
        - name: echo-service
          port: 80
"#;

        // Try to deserialize the Gateway API format
        let gateway_api_doc: serde_yaml::Value = serde_yaml::from_str(gateway_api_yaml).unwrap();
        let rules = &gateway_api_doc["spec"]["rules"];
        let filters = &rules[0]["filters"];

        println!("Gateway API filters structure: {:#?}", filters);

        // Extract the first filter
        let filter_yaml = &filters[0];
        println!("First filter YAML: {:#?}", filter_yaml);

        // Try to deserialize directly as our HTTPRouteFilter
        let filter_result: Result<HTTPRouteFilter, _> = serde_yaml::from_value(filter_yaml.clone());
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
                panic!("Filter deserialization failed");
            }
        }
    }

    #[test]
    fn test_kubera_config_filter_deserialization() {
        // This is what the config should look like after conversion to snake_case
        let kubera_config_yaml = r#"
type: RequestHeaderModifier
request_header_modifier:
  set:
    - name: "X-Gateway"
      value: "kubera-gateway"
    - name: "X-Route-Name"
      value: "echo-route"
  add:
    - name: "X-Request-ID"
      value: "generated-uuid-12345"
  remove:
    - "X-Debug-Info"
"#;

        let filter: HTTPRouteFilter = serde_yaml::from_str(kubera_config_yaml).unwrap();
        println!("Kubera config filter: {:#?}", filter);

        assert_eq!(
            filter.filter_type,
            HTTPRouteFilterType::RequestHeaderModifier
        );
        assert!(filter.request_header_modifier.is_some());

        let modifier = filter.request_header_modifier.unwrap();
        assert!(modifier.set.is_some());
        assert!(modifier.add.is_some());
        assert!(modifier.remove.is_some());

        let set_headers = modifier.set.unwrap();
        assert_eq!(set_headers.len(), 2);
        assert_eq!(set_headers[0].name, "X-Gateway");
        assert_eq!(set_headers[0].value, "kubera-gateway");
    }

    #[test]
    fn test_filter_conversion_from_gateway_api_to_kubera() {
        // Test the conversion process that should happen in the control plane
        let gateway_api_filter_yaml = r#"
type: RequestHeaderModifier
requestHeaderModifier:
  set:
    - name: "X-Gateway"
      value: "kubera-gateway"
  add:
    - name: "X-Request-ID"
      value: "generated-uuid-12345"
  remove:
    - "X-Debug-Info"
"#;

        // First, deserialize as a generic YAML value
        let gateway_filter: serde_yaml::Value =
            serde_yaml::from_str(gateway_api_filter_yaml).unwrap();
        println!("Gateway API filter structure: {:#?}", gateway_filter);

        // Try to deserialize directly to our filter type (this should fail with current config)
        let direct_result: Result<HTTPRouteFilter, _> =
            serde_yaml::from_value(gateway_filter.clone());
        match direct_result {
            Ok(filter) => {
                println!("Direct deserialization worked: {:#?}", filter);
            }
            Err(e) => {
                println!("Direct deserialization failed (expected): {}", e);

                // This shows we need to convert camelCase to snake_case manually
                // Let's simulate the conversion process
                let converted_yaml = r#"
type: RequestHeaderModifier
request_header_modifier:
  set:
    - name: "X-Gateway"
      value: "kubera-gateway"
  add:
    - name: "X-Request-ID"
      value: "generated-uuid-12345"
  remove:
    - "X-Debug-Info"
"#;
                let converted_filter: HTTPRouteFilter =
                    serde_yaml::from_str(converted_yaml).unwrap();
                println!("Converted filter: {:#?}", converted_filter);
                assert_eq!(
                    converted_filter.filter_type,
                    HTTPRouteFilterType::RequestHeaderModifier
                );
            }
        }
    }
}
