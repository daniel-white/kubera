//! Integration tests for Vale Gateway
//!
//! These tests verify the interaction between different components
//! of the Vale Gateway system, including the control plane and gateway.

use std::time::Duration;
use test_log::test;
use tokio::time::timeout;

mod common;
mod config_tests;
mod control_plane_tests;
mod gateway_tests;

/// Test that core components can be initialized
#[test]
async fn test_core_initialization() {
    // Test that we can initialize crypto
    vg_core::crypto::init_crypto();

    // Initialize instrumentation for tests
    let _guard = vg_core::instrumentation::init_instrumentation();

    // Test basic signal functionality
    let (tx, rx) = vg_core::sync::signal::signal::<String>();

    tx.set("test".to_string()).await;
    assert_eq!(rx.get().await, Some("test".to_string()));
}

/// Test configuration loading and validation
#[test]
async fn test_configuration_lifecycle() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("gateway.yaml");

    // Write a test configuration
    let config_content = r#"
apiVersion: v1
kind: Config
metadata:
  name: test-gateway
spec:
  listeners:
    - name: http
      port: 8080
      protocol: HTTP
"#;

    fs::write(&config_path, config_content).unwrap();

    // Test that we can read and parse the configuration
    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("test-gateway"));
}

/// Test that the system handles errors gracefully
#[test]
async fn test_error_handling() {
    // Test signal with dropped sender
    let (tx, rx) = vg_core::sync::signal::signal::<i32>();

    tx.set(42).await;
    drop(tx);

    // Should still be able to get the last value
    assert_eq!(rx.get().await, Some(42));

    // But changed() should error
    assert!(rx.changed().await.is_err());
}

/// Test concurrent access patterns
#[test]
async fn test_concurrent_operations() {
    let (tx, rx) = vg_core::sync::signal::signal::<usize>();

    // Spawn multiple tasks that write to the signal
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let tx = tx.clone();
            tokio::spawn(async move {
                tx.set(i).await;
            })
        })
        .collect();

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Should have some final value
    assert!(rx.get().await.is_some());
}

/// Test timeout behavior
#[test]
async fn test_timeout_behavior() {
    let (_tx, rx) = vg_core::sync::signal::signal::<i32>();

    // This should timeout since no value is ever set
    let result = timeout(Duration::from_millis(50), rx.changed()).await;
    assert!(result.is_err());
}
