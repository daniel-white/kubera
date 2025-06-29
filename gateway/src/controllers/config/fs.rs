use getset::Getters;
use kubera_core::config::gateway::serde::read_configuration;
use kubera_core::config::gateway::types::GatewayConfiguration;
use kubera_core::continue_on;
use kubera_core::io::file_watcher::spawn_file_watcher;
use kubera_core::sync::signal::{channel, Receiver};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::fs::read;
use tokio::task::JoinSet;
use tracing::{debug, info};

#[derive(Debug, Getters)]
pub struct WatchConfigurationFileParams {
    file_path: PathBuf,
}

impl WatchConfigurationFileParams {
    pub fn new_builder() -> WatchConfigurationFileParamsBuilder {
        WatchConfigurationFileParamsBuilder::default()
    }
}

pub struct WatchConfigurationFileParamsBuilder {
    file_path: Option<PathBuf>,
}

impl Default for WatchConfigurationFileParamsBuilder {
    fn default() -> Self {
        Self { file_path: None }
    }
}

impl WatchConfigurationFileParamsBuilder {
    pub fn file_path<P: AsRef<Path>>(&mut self, p: P) -> &mut Self {
        self.file_path = Some(PathBuf::from(p.as_ref()));
        self
    }

    pub fn build(self) -> WatchConfigurationFileParams {
        WatchConfigurationFileParams {
            file_path: self.file_path.expect("file_path must be set"),
        }
    }
}

pub fn watch_configuration_file(
    join_set: &mut JoinSet<()>,
    params: WatchConfigurationFileParams,
) -> Receiver<Option<(Instant, GatewayConfiguration)>> {
    let (tx, rx) = channel(None);

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
                    info!("Configuration file changed");
                    tx.replace(Some((serial, config)));
                }
            }
            
            continue_on!(file_watcher.changed());
        }
    });

    rx
}
