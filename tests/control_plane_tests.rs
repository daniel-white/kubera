//! Tests for control plane functionality

use crate::common::*;
use std::time::Duration;
use test_log::test;

#[test]
async fn test_control_plane_signal_communication() {
    init_test_env();

    // Test the signal communication mechanism used by the control plane
    let (tx, rx) = kubera_core::sync::signal::signal::<String>();

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
    let (tx, rx1) = kubera_core::sync::signal::signal::<i32>();
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

    let (tx, rx) = kubera_core::sync::signal::signal::<String>();

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

    let (tx, rx) = kubera_core::sync::signal::signal::<usize>();

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
