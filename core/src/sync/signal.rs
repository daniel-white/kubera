use anyhow::Result;
use atomic_refcell::AtomicRefCell;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::sync::broadcast::{Receiver as BroadcastReceiver, Sender as BroadcastSender, channel};
use tracing::trace;

#[derive(Debug, Error)]
#[error("Receiver error")]
pub struct RecvError;

pub fn signal<T: PartialEq + Clone>() -> (Sender<T>, Receiver<T>) {
    let data = Arc::new(RwLock::new(None));
    let (tx, rx) = channel(10);
    (
        Sender {
            data: data.clone(),
            tx: tx.clone(),
        },
        Receiver {
            tx,
            rx: AtomicRefCell::new(rx),
            data,
        },
    )
}

#[derive(Clone, Debug)]
pub struct Sender<T: PartialEq + Clone> {
    data: Arc<RwLock<Option<T>>>,
    tx: BroadcastSender<()>,
}

impl<T: PartialEq + Clone> Sender<T> {
    pub async fn get(&self) -> Option<T> {
        self.data.read().await.clone()
    }

    pub async fn set(&self, value: T) {
        let value = match self.data.read().await.as_ref() {
            Some(old_value) if old_value != &value => {
                trace!("Replacing value in signal");
                Some(value)
            }
            None => {
                trace!("Setting value in signal");
                Some(value)
            }
            _ => {
                trace!("No change in value, not updating signal");
                None
            }
        };

        if let Some(value) = value {
            self.data.write().await.replace(value);
            let _ = self.tx.send(());
        }
    }

    pub async fn clear(&self) {
        if self.data.read().await.is_some() {
            trace!("Clearing value in signal");
            self.data.write().await.take();
            let _ = self.tx.send(());
        } else {
            trace!("No value to clear in signal");
        }
    }

    pub async fn replace(&self, value: Option<T>) {
        if self.data.read().await.as_ref() == value.as_ref() {
            trace!("No change in value, not updating signal");
        } else if let Some(value) = value {
            trace!("Replacing value in signal");
            self.data.write().await.replace(value);
            let _ = self.tx.send(());
        } else {
            trace!("Clearing value in signal");
            self.data.write().await.take();
            let _ = self.tx.send(());
        }
    }
}

#[derive(Debug)]
pub struct Receiver<T: PartialEq + Clone> {
    tx: BroadcastSender<()>,
    rx: AtomicRefCell<BroadcastReceiver<()>>,
    data: Arc<RwLock<Option<T>>>,
}

impl<T: PartialEq + Clone> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        let rx = self.tx.subscribe();
        Receiver {
            tx: self.tx.clone(),
            rx: AtomicRefCell::new(rx),
            data: self.data.clone(),
        }
    }
}

impl<T: PartialEq + Clone> Receiver<T> {
    pub async fn get(&self) -> Option<T> {
        self.data.read().await.clone()
    }

    pub async fn changed(&self) -> Result<(), RecvError> {
        self.rx.borrow_mut().recv().await.map_err(|_| {
            trace!("Sender dropped");
            RecvError
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assertables::assert_ok;
    use proptest::prelude::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::{Duration, timeout};
    use tokio_test::{assert_pending, assert_ready};

    #[tokio::test]
    async fn test_signal_channel() {
        let (tx, rx) = signal();
        assert_eq!(rx.get().await, None);

        tx.set(43).await;
        assert_eq!(rx.get().await, Some(43));

        tx.clear().await;
        assert_eq!(rx.get().await, None);

        tx.set(44).await;
        assert_eq!(rx.get().await, Some(44));

        tx.set(44).await; // No change, should not update
        assert_eq!(rx.get().await, Some(44));
    }

    #[tokio::test]
    async fn test_signal_notification() {
        let (tx, rx) = signal();

        // Should not block when no value is set
        let mut changed_future = std::pin::pin!(rx.changed());
        assert_pending!(tokio_test::task::spawn(&mut changed_future).poll());

        // Set a value and check notification
        tx.set(42).await;
        assert_ready!(tokio_test::task::spawn(&mut changed_future).poll()).unwrap();

        // Check we can receive the value
        assert_eq!(rx.get().await, Some(42));
    }

    #[tokio::test]
    async fn test_multiple_receivers() {
        let (tx, rx1) = signal();
        let rx2 = rx1.clone();
        let rx3 = rx1.clone();

        tx.set("hello".to_string()).await;

        assert_eq!(rx1.get().await, Some("hello".to_string()));
        assert_eq!(rx2.get().await, Some("hello".to_string()));
        assert_eq!(rx3.get().await, Some("hello".to_string()));
    }

    #[tokio::test]
    async fn test_receiver_notifications_multiple() {
        let (tx, rx) = signal();
        let rx2 = rx.clone();

        let notify_count = Arc::new(AtomicUsize::new(0));
        let notify_count_clone = notify_count.clone();

        // Use a different approach - collect notifications until we expect to be done
        let handle = tokio::spawn(async move {
            let mut count = 0;
            while count < 4 {
                // We expect exactly 4 notifications
                if rx2.changed().await.is_ok() {
                    notify_count_clone.fetch_add(1, Ordering::SeqCst);
                    count += 1;
                } else {
                    break; // Sender dropped
                }
            }
        });

        // Send multiple different values
        tx.set(1).await;
        tx.set(2).await;
        tx.set(3).await;
        tx.set(3).await; // Same value, should not notify
        tx.set(4).await;

        // Wait for all notifications to be processed
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Complete the test by finishing the spawned task
        let result = tokio::time::timeout(Duration::from_millis(100), handle).await;
        assert!(result.is_ok(), "Test should complete within timeout");

        assert_eq!(notify_count.load(Ordering::SeqCst), 4); // Should be 4, not 5
    }

    #[tokio::test]
    async fn test_sender_dropped() {
        let (tx, rx) = signal();

        tx.set(100).await;
        assert_eq!(rx.get().await, Some(100));

        // Clone the receiver to test multiple receivers behavior when sender drops
        let rx2 = rx.clone();

        // Drop the sender
        drop(tx);

        // The key behavior: get() should still return the last value
        assert_eq!(rx.get().await, Some(100));
        assert_eq!(rx2.get().await, Some(100));

        // Test that we can't set new values (no sender exists)
        // This is the important behavior for your use case
        // Since we dropped all senders, no new values can be set
        // The receivers should maintain the last known value
    }

    #[tokio::test]
    async fn test_clear_functionality() {
        let (tx, rx) = signal();

        tx.set(42).await;
        assert_eq!(rx.get().await, Some(42));

        tx.clear().await;
        assert_eq!(rx.get().await, None);

        // Setting after clear should work
        tx.set(84).await;
        assert_eq!(rx.get().await, Some(84));
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let (tx, rx) = signal();
        let tx_clone = tx.clone();

        let handles = (0..10)
            .map(|i| {
                let tx = if i % 2 == 0 {
                    tx.clone()
                } else {
                    tx_clone.clone()
                };
                tokio::spawn(async move {
                    tx.set(i).await;
                })
            })
            .collect::<Vec<_>>();

        // Wait for all tasks to complete
        for handle in handles {
            assert_ok!(handle.await);
        }

        // Should have some value (the last one set)
        assert!(rx.get().await.is_some());
    }

    #[tokio::test]
    async fn test_timeout_on_changed() {
        let (_tx, rx) = signal::<i32>();

        // Should timeout since no value is ever set
        let result = timeout(Duration::from_millis(10), rx.changed()).await;
        assert!(result.is_err()); // Timeout error
    }

    proptest! {
        #[test]
        fn test_signal_properties(values in prop::collection::vec(any::<i32>(), 0..20)) {
            let runtime = assert_ok!(tokio::runtime::Runtime::new());
            runtime.block_on(async {
                let (tx, rx) = signal();

                let mut last_value = None;
                for value in &values {
                    tx.set(*value).await;
                    last_value = Some(*value);
                }

                if let Some(expected) = last_value {
                    prop_assert_eq!(rx.get().await, Some(expected));
                } else {
                    prop_assert_eq!(rx.get().await, None);
                }

                Ok(())
            })?;
        }
    }
}
