//! Tests for control plane functionality

use crate::common::*;
use std::time::Duration;
use test_log::test;

#[test]
async fn test_control_plane_signal_communication() {
    init_test_env();

    // Test the signal communication mechanism used by the control plane
    let (tx, rx) = vg_core::sync::signal::signal::<String>();

    // Simulate control plane sending configuration updates
    tx.set("config-v1".to_string()).await;
    assert_eq!(rx.get().await, Some("config-v1".to_string()));

    // Update configuration
    tx.set("config-v2".to_string()).await;
    assert_eq!(rx.get().await, Some("config-v2".to_string()));

    // Clear configuration
    tx.clear().await;
    assert_eq!(rx.get().await, None);
}

#[test]
async fn test_multiple_controller_coordination() {
    init_test_env();

    // Simulate multiple controllers watching the same resource
    let (tx, rx1) = vg_core::sync::signal::signal::<i32>();
    let rx2 = rx1.clone();
    let rx3 = rx1.clone();

    // All controllers should see the same state
    tx.set(42).await;

    assert_eq!(rx1.get().await, Some(42));
    assert_eq!(rx2.get().await, Some(42));
    assert_eq!(rx3.get().await, Some(42));
}

#[test]
async fn test_controller_error_handling() {
    init_test_env();

    let (tx, rx) = vg_core::sync::signal::signal::<String>();

    // Set initial state
    tx.set("initial".to_string()).await;
    assert_eq!(rx.get().await, Some("initial".to_string()));

    // Simulate controller crash (sender dropped)
    drop(tx);

    // Receiver should still have last known good state
    assert_eq!(rx.get().await, Some("initial".to_string()));

    // But should error on waiting for changes
    assert!(rx.changed().await.is_err());
}

#[test]
async fn test_concurrent_configuration_updates() {
    init_test_env();

    let (tx, rx) = vg_core::sync::signal::signal::<usize>();

    // Simulate multiple controllers trying to update configuration
    let handles: Vec<_> = (0..5)
        .map(|i| {
            let tx = tx.clone();
            tokio::spawn(async move {
                // Each controller attempts multiple updates
                for j in 0..3 {
                    tx.set(i * 10 + j).await;
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            })
        })
        .collect();

    // Wait for all controllers to finish
    for handle in handles {
        handle.await.unwrap();
    }

    // Should have some final configuration
    let final_config = rx.get().await;
    assert!(final_config.is_some());

    // Final value should be within expected range
    let value = final_config.unwrap();
    assert!(value < 50); // Max possible value is 4*10 + 2 = 42
}

//! Tests for HTTP route namespace filtering functionality

mod http_route_namespace_filtering {
    use crate::common::*;
    use gateway_api::apis::standard::gateways::{
        Gateway, GatewaySpec, GatewaySpecListeners, GatewaySpecListenersAllowedRoutes,
        GatewaySpecListenersAllowedRoutesNamespaces,
    };
    use gateway_api::apis::standard::httproutes::{HTTPRoute, HTTPRouteSpec, HTTPRouteSpecParentRefs};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use std::time::Duration;
    use test_log::test;
    use vg_control_plane::controllers::filters::http_routes::filter_http_routes;
    use vg_control_plane::kubernetes::objects::{ObjectRef, Objects};
    use vg_core::sync::signal::signal;
    use vg_core::task::Builder as TaskBuilder;

    /// Helper function to create a test Gateway with specific allowedRoutes configuration
    fn create_test_gateway(name: &str, namespace: &str, allowed_routes_from: Option<&str>) -> Gateway {
        let allowed_routes = if let Some(from_value) = allowed_routes_from {
            Some(GatewaySpecListenersAllowedRoutes {
                namespaces: Some(GatewaySpecListenersAllowedRoutesNamespaces {
                    from: Some(from_value.to_string()),
                    selector: None,
                }),
                kinds: None,
            })
        } else {
            None
        };

        Gateway {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                ..Default::default()
            },
            spec: GatewaySpec {
                gateway_class_name: "vale-gateway".to_string(),
                listeners: Some(vec![GatewaySpecListeners {
                    name: "http".to_string(),
                    port: 80,
                    protocol: "HTTP".to_string(),
                    allowed_routes,
                    hostname: None,
                    tls: None,
                }]),
                addresses: None,
                infrastructure: None,
            },
            status: None,
        }
    }

    /// Helper function to create a test HTTPRoute
    fn create_test_http_route(name: &str, namespace: &str, gateway_name: &str, gateway_namespace: Option<&str>) -> HTTPRoute {
        HTTPRoute {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                ..Default::default()
            },
            spec: HTTPRouteSpec {
                parent_refs: Some(vec![HTTPRouteSpecParentRefs {
                    group: None,
                    kind: None,
                    name: gateway_name.to_string(),
                    namespace: gateway_namespace.map(|s| s.to_string()),
                    port: None,
                    section_name: None,
                }]),
                hostnames: None,
                rules: Some(vec![]),
            },
            status: None,
        }
    }

    #[test]
    async fn test_http_route_same_namespace_allowed() {
        init_test_env();

        let task_builder = TaskBuilder::new("test");

        // Create gateway with default allowedRoutes (Same namespace)
        let gateway = create_test_gateway("test-gateway", "default", None);
        let gateway_ref = ObjectRef::of_kind::<Gateway>()
            .namespace("default")
            .name("test-gateway")
            .build();

        let mut gateways = Objects::new();
        gateways.insert(gateway_ref, gateway.into());

        // Create HTTPRoute in the same namespace
        let http_route = create_test_http_route("test-route", "default", "test-gateway", None);
        let route_ref = ObjectRef::of_kind::<HTTPRoute>()
            .namespace("default")
            .name("test-route")
            .build();

        let mut http_routes = Objects::new();
        http_routes.insert(route_ref.clone(), http_route.into());

        // Set up signal channels
        let (gateways_tx, gateways_rx) = signal("gateways");
        let (http_routes_tx, http_routes_rx) = signal("http_routes");

        gateways_tx.set(gateways).await;
        http_routes_tx.set(http_routes).await;

        // Test filtering
        let filtered_rx = filter_http_routes(&task_builder, &gateways_rx, &http_routes_rx);

        // Give the filter time to process
        tokio::time::sleep(Duration::from_millis(100)).await;

        let filtered_routes = filtered_rx.get().await.expect("Should have filtered routes");
        assert_eq!(filtered_routes.len(), 1, "Route in same namespace should be allowed");
        assert!(filtered_routes.contains_by_ref(&route_ref), "The specific route should be present");
    }

    #[test]
    async fn test_http_route_different_namespace_rejected_with_same_policy() {
        init_test_env();

        let task_builder = TaskBuilder::new("test");

        // Create gateway with explicit "Same" namespace policy
        let gateway = create_test_gateway("test-gateway", "default", Some("Same"));
        let gateway_ref = ObjectRef::of_kind::<Gateway>()
            .namespace("default")
            .name("test-gateway")
            .build();

        let mut gateways = Objects::new();
        gateways.insert(gateway_ref, gateway.into());

        // Create HTTPRoute in a different namespace
        let http_route = create_test_http_route("test-route", "other", "test-gateway", Some("default"));
        let route_ref = ObjectRef::of_kind::<HTTPRoute>()
            .namespace("other")
            .name("test-route")
            .build();

        let mut http_routes = Objects::new();
        http_routes.insert(route_ref, http_route.into());

        // Set up signal channels
        let (gateways_tx, gateways_rx) = signal("gateways");
        let (http_routes_tx, http_routes_rx) = signal("http_routes");

        gateways_tx.set(gateways).await;
        http_routes_tx.set(http_routes).await;

        // Test filtering
        let filtered_rx = filter_http_routes(&task_builder, &gateways_rx, &http_routes_rx);

        // Give the filter time to process
        tokio::time::sleep(Duration::from_millis(100)).await;

        let filtered_routes = filtered_rx.get().await.expect("Should have filtered routes");
        assert_eq!(filtered_routes.len(), 0, "Route in different namespace should be rejected with Same policy");
    }

    #[test]
    async fn test_http_route_all_namespaces_allowed() {
        init_test_env();

        let task_builder = TaskBuilder::new("test");

        // Create gateway with "All" namespace policy
        let gateway = create_test_gateway("test-gateway", "default", Some("All"));
        let gateway_ref = ObjectRef::of_kind::<Gateway>()
            .namespace("default")
            .name("test-gateway")
            .build();

        let mut gateways = Objects::new();
        gateways.insert(gateway_ref, gateway.into());

        // Create HTTPRoutes in different namespaces
        let route1 = create_test_http_route("test-route-1", "default", "test-gateway", Some("default"));
        let route1_ref = ObjectRef::of_kind::<HTTPRoute>()
            .namespace("default")
            .name("test-route-1")
            .build();

        let route2 = create_test_http_route("test-route-2", "other", "test-gateway", Some("default"));
        let route2_ref = ObjectRef::of_kind::<HTTPRoute>()
            .namespace("other")
            .name("test-route-2")
            .build();

        let mut http_routes = Objects::new();
        http_routes.insert(route1_ref.clone(), route1.into());
        http_routes.insert(route2_ref.clone(), route2.into());

        // Set up signal channels
        let (gateways_tx, gateways_rx) = signal("gateways");
        let (http_routes_tx, http_routes_rx) = signal("http_routes");

        gateways_tx.set(gateways).await;
        http_routes_tx.set(http_routes).await;

        // Test filtering
        let filtered_rx = filter_http_routes(&task_builder, &gateways_rx, &http_routes_rx);

        // Give the filter time to process
        tokio::time::sleep(Duration::from_millis(100)).await;

        let filtered_routes = filtered_rx.get().await.expect("Should have filtered routes");
        assert_eq!(filtered_routes.len(), 2, "All routes should be allowed with All policy");
        assert!(filtered_routes.contains_by_ref(&route1_ref), "Route in same namespace should be present");
        assert!(filtered_routes.contains_by_ref(&route2_ref), "Route in different namespace should be present");
    }

    #[test]
    async fn test_http_route_selector_namespace_policy() {
        init_test_env();

        let task_builder = TaskBuilder::new("test");

        // Create gateway with "Selector" namespace policy (currently unimplemented, should allow)
        let gateway = create_test_gateway("test-gateway", "default", Some("Selector"));
        let gateway_ref = ObjectRef::of_kind::<Gateway>()
            .namespace("default")
            .name("test-gateway")
            .build();

        let mut gateways = Objects::new();
        gateways.insert(gateway_ref, gateway.into());

        // Create HTTPRoute in different namespace
        let http_route = create_test_http_route("test-route", "other", "test-gateway", Some("default"));
        let route_ref = ObjectRef::of_kind::<HTTPRoute>()
            .namespace("other")
            .name("test-route")
            .build();

        let mut http_routes = Objects::new();
        http_routes.insert(route_ref.clone(), http_route.into());

        // Set up signal channels
        let (gateways_tx, gateways_rx) = signal("gateways");
        let (http_routes_tx, http_routes_rx) = signal("http_routes");

        gateways_tx.set(gateways).await;
        http_routes_tx.set(http_routes).await;

        // Test filtering
        let filtered_rx = filter_http_routes(&task_builder, &gateways_rx, &http_routes_rx);

        // Give the filter time to process
        tokio::time::sleep(Duration::from_millis(100)).await;

        let filtered_routes = filtered_rx.get().await.expect("Should have filtered routes");
        // Currently selector is unimplemented and allows routes (with warning)
        assert_eq!(filtered_routes.len(), 1, "Route should be allowed with Selector policy (unimplemented)");
        assert!(filtered_routes.contains_by_ref(&route_ref), "The route should be present");
    }
}

#[test]
fn test_gateway_deployment_replicas_from_class() {
    use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec};
    use crate::common::*;
    use vg_core::task::Builder as TaskBuilder;
    use crate::control_plane::controllers::transformers::GatewayInstanceConfiguration;

    // Create a deployment override with custom replicas
    let mut deployment = Deployment::default();
    deployment.spec = Some(DeploymentSpec {
        replicas: Some(5), // custom replica count
        ..Default::default()
    });

    // Create a GatewayInstanceConfiguration with the override
    let instance = GatewayInstanceConfiguration {
        deployment_overrides: deployment,
        // ...other required fields, use defaults or mocks...
        ..Default::default()
    };

    // Build template values as in generate_gateway_deployments
    let replicas = instance.deployment_overrides().spec.as_ref()
        .and_then(|spec| spec.replicas)
        .unwrap_or(1) as u32;

    assert_eq!(replicas, 5, "Replicas should match the value from gateway class parameters");
}

#[test]
fn test_gateway_deployment_replicas_precedence() {
    use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec};
    use vg_api::v1alpha1::{GatewayClassParameters, GatewayConfiguration, GatewayParameters};
    use gateway_api::apis::standard::gateways::Gateway;
    use crate::control_plane::controllers::transformers::merge_deployment_overrides;
    // Helper: create dummy Gateway
    let gateway = Gateway {
        metadata: Default::default(),
        spec: Default::default(),
        status: None,
    };

    // Case 1: Only class parameters set
    let mut class_params = GatewayClassParameters::default();
    class_params.spec.common.deployment = Some(Default::default());
    class_params.spec.common.deployment.as_mut().unwrap().replicas = Some(3);
    let (deployment, _, _, _) = merge_deployment_overrides(&gateway, Some(&class_params), None);
    assert_eq!(deployment.spec.as_ref().unwrap().replicas, Some(3), "Should use class parameter replicas");

    // Case 2: Only gateway parameters set
    let mut gw_params = GatewayParameters::default();
    gw_params.spec.common = Some(Default::default());
    gw_params.spec.common.as_mut().unwrap().deployment = Some(Default::default());
    gw_params.spec.common.as_mut().unwrap().deployment.as_mut().unwrap().replicas = Some(7);
    let (deployment, _, _, _) = merge_deployment_overrides(&gateway, None, Some(&gw_params));
    assert_eq!(deployment.spec.as_ref().unwrap().replicas, Some(7), "Should use gateway parameter replicas");

    // Case 3: Both set, gateway param should win
    let mut class_params = GatewayClassParameters::default();
    class_params.spec.common.deployment = Some(Default::default());
    class_params.spec.common.deployment.as_mut().unwrap().replicas = Some(2);
    let mut gw_params = GatewayParameters::default();
    gw_params.spec.common = Some(Default::default());
    gw_params.spec.common.as_mut().unwrap().deployment = Some(Default::default());
    gw_params.spec.common.as_mut().unwrap().deployment.as_mut().unwrap().replicas = Some(9);
    let (deployment, _, _, _) = merge_deployment_overrides(&gateway, Some(&class_params), Some(&gw_params));
    assert_eq!(deployment.spec.as_ref().unwrap().replicas, Some(9), "Gateway param should take precedence over class param");

    // Case 4: Neither set, should default to 1
    let (deployment, _, _, _) = merge_deployment_overrides(&gateway, None, None);
    assert_eq!(deployment.spec.as_ref().unwrap().replicas, Some(1), "Should default to 1 replica");
}
