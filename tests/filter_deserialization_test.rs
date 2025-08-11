use vg_core::config::gateway::types::http::filters::{
    HttpRouteFilter, HttpRouteFilterType, RequestHeaderModifier,
};

#[cfg(test)]
mod filter_deserialization_tests {
    use serde_yaml;

    #[test]
    fn test_gateway_api_filter_deserialization() {
        // This is what comes from Gateway API HTTPRoute
        let gateway_api_yaml = r#"
type: RequestHeaderModifier
requestHeaderModifier:
  set:
    - name: vale-gateway
      value: "test"
  add:
    - name: X-Request-ID
      value: "generated-uuid-12345"
  remove:
    - X-Debug-Info
"#;

        let filter: HttpRouteFilter = serde_yaml::from_str(gateway_api_yaml).unwrap();

        assert_eq!(
            filter.filter_type,
            HttpRouteFilterType::RequestHeaderModifier
        );
        assert!(filter.request_header_modifier.is_some());

        let modifier = filter.request_header_modifier.as_ref().unwrap();
        assert_eq!(modifier.set().len(), 1);
        assert_eq!(modifier.set()[0].name(), "vale-gateway");
        assert_eq!(modifier.set()[0].value(), "test");

        assert_eq!(modifier.add().len(), 1);
        assert_eq!(modifier.add()[0].name(), "X-Request-ID");
        assert_eq!(modifier.add()[0].value(), "generated-uuid-12345");

        assert_eq!(modifier.remove().len(), 1);
        assert_eq!(modifier.remove()[0], "X-Debug-Info");
    }

    #[test]
    fn test_vg_config_filter_deserialization() {
        // This is what a Vale Gateway config.yaml should look like
        let vg_config_yaml = r#"
type: "RequestHeaderModifier"
request_header_modifier:
  set:
    - name: "X-Gateway"
      value: "vale-gateway"
  add:
    - name: "X-Request-ID"
      value: "test-id"
"#;

        let filter: HttpRouteFilter = serde_yaml::from_str(vg_config_yaml).unwrap();
        assert_eq!(
            filter.filter_type,
            HttpRouteFilterType::RequestHeaderModifier
        );
        assert!(filter.request_header_modifier.is_some());

        let modifier = filter.request_header_modifier.unwrap();
        let set_headers = modifier.set();
        assert_eq!(set_headers.len(), 1);
        assert_eq!(set_headers[0].name(), "X-Gateway");
        assert_eq!(set_headers[0].value(), "vale-gateway");
    }

    #[test]
    fn test_filter_conversion_from_gateway_api_to_vale_gateway() {
        // Input: Gateway API format
        let gateway_api_yaml = r#"
type: RequestHeaderModifier
requestHeaderModifier:
  set:
    - name: X-Gateway
      value: "vale-gateway"
  add:
    - name: X-Request-ID
      value: "12345"
  remove:
    - X-Debug
"#;

        // Output: Vale Gateway internal format
        let vg_yaml = r#"
type: "RequestHeaderModifier"
request_header_modifier:
  set:
    - name: "X-Gateway"
      value: "vale-gateway"
  add:
    - name: "X-Request-ID"
      value: "12345"
  remove:
    - "X-Debug"
"#;
    }
}
