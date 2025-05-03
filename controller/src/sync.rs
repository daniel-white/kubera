use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio::sync::RwLock;

pub struct State<T>
where
    T: Clone,
{
    state: RwLock<Option<T>>,
    tx: Sender<Option<T>>,
    rx: RwLock<Receiver<Option<T>>>,
}

impl<T> State<T>
where
    T: Clone,
{
    pub fn new() -> Self {
        let (tx, rx) = channel(16);
        Self {
            state: RwLock::new(None),
            tx,
            rx: RwLock::new(rx),
        }
    }

    pub async fn set(&self, new_state: T) {
        let mut state = self.state.write().await;
        *state = Some(new_state);
        let _ = self.tx.send(Some(new_state));
    }

    pub async fn unset(&self) {
        let mut state = self.state.write().await;
        *state = None;
        let _ = self.tx.send(());
    }

    pub async fn current(&self) -> impl Deref<Target = Option<T>> + '_ {
        self.state.read().await
    }

    pub async fn recv(&self) -> Option<T> {
        let mut rx = self.rx.write().await;
        rx.recv().await
    }
}

pub type SharedState<T> = Arc<State<T>>;

pub fn shared<T>() -> SharedState<T> {
    SharedState::new(State::new())
}

#[cfg(test)]
mod tests {
    use tokio::select;
    use tokio::sync::Mutex;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_watchable_changed() {
        let state = Arc::new(State::new());
        let signal_called_count = Arc::new(Mutex::new(0));

        // Initially, the state should be None
        assert_eq!(*state.current().await, None);

        // Set a value and ensure `changed` fires
        let signal_called_count_clone = signal_called_count.clone();
        let state_clone = state.clone();
        let handle = tokio::spawn(async move {
            select! {
                _ = state_clone.signal() => {
                    assert_eq!(*state_clone.current().await, Some(42));
                    let mut count = signal_called_count_clone.lock().await;
                    *count += 1;
                },
            }
        });
        state.set(42).await;
        if let Err(e) = timeout(Duration::from_secs(1), async { handle.await.unwrap() }).await {
            panic!("Timeout or error occurred: {:?}", e);
        }
        assert_eq!(*state.current().await, Some(42));

        // Unset the value and ensure `changed` fires again
        let signal_called_count_clone = signal_called_count.clone();
        let state_clone = state.clone();
        let handle = tokio::spawn(async move {
            select! {
                _ = state_clone.signal() => {
                    assert_eq!(*state_clone.current().await, None);
                    let mut count = signal_called_count_clone.lock().await;
                    *count += 1;
                },
            }
        });

        state.unset().await;
        if let Err(e) = timeout(Duration::from_secs(1), async { handle.await.unwrap() }).await {
            panic!("Timeout or error occurred: {:?}", e);
        }
        assert_eq!(*state.current().await, None);

        // Assert the total number of changes
        let count = signal_called_count.lock().await;
        assert_eq!(*count, 2);
    }
}
