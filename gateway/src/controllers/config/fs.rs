use getset::Getters;
use kubera_core::config::gateway::serde::read_configuration;
use kubera_core::config::gateway::types::GatewayConfiguration;
use kubera_core::continue_after;
use kubera_core::io::file_watcher::spawn_file_watcher;
use kubera_core::sync::signal::{Receiver, signal};
use std::io::Cursor;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::fs::read;
use tokio::task::JoinSet;
use tracing::{debug, info};
use typed_builder::TypedBuilder;

#[derive(Debug, Getters, TypedBuilder)]
pub struct WatchConfigurationFileParams {
    #[builder(setter(into))]
    file_path: PathBuf,
}

pub fn watch_configuration_file(
    join_set: &mut JoinSet<()>,
    params: WatchConfigurationFileParams,
) -> Receiver<(Instant, GatewayConfiguration)> {
    let (tx, rx) = signal();

    join_set.spawn(async move {
        info!(
            "Spawning file watcher for configuration file: {:?}",
            params.file_path
        );

        let file_watcher =
            spawn_file_watcher(&params.file_path).expect("Failed to spawn file watcher");

        loop {
            let serial = Instant::now();

            if let Ok(config_reader) = read(&params.file_path).await.map(Cursor::new) {
                if let Ok(config) = read_configuration(config_reader) {
                    debug!("Configuration file read");
                    tx.set((serial, config)).await;
                }
            }

            continue_after!(
                Duration::from_secs(30), // failsafe timeout to force a re-read
                file_watcher.changed()
            );
        }
    });

    rx
}
