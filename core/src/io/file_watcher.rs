use crate::sync::signal::{signal, Receiver, Sender};
use anyhow::Result;
use notify::{Event, EventHandler, RecursiveMode, Watcher};
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{debug, info};

struct SignalEventHandler {
    tx: Sender<u64>,
    generation: AtomicU64,
}

impl SignalEventHandler {
    fn new(tx: Sender<u64>) -> Self {
        Self {
            tx,
            generation: AtomicU64::new(0),
        }
    }

    fn increment_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::Relaxed)
    }
}

impl EventHandler for SignalEventHandler {
    fn handle_event(&mut self, event: notify::Result<Event>) {
        if let Ok(event) = event {
            debug!("File watcher event: {:?}", event);
            if event.kind.is_modify() || event.kind.is_create() {
                let generation = self.increment_generation();
                self.tx.set(generation);
            }
        }
    }
}

pub fn spawn_file_watcher<P: AsRef<std::path::Path>>(p: P) -> Result<Receiver<u64>> {
    let (tx, rx) = signal();

    let mut watcher = notify::recommended_watcher(SignalEventHandler::new(tx))?;
    watcher.watch(p.as_ref(), RecursiveMode::NonRecursive)?;
    Box::leak(Box::new(watcher));

    info!("Started file watcher for: {:?}", p.as_ref());

    Ok(rx)
}
