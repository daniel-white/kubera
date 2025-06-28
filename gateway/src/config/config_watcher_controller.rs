use anyhow::Result;
use derive_builder::Builder;
use getset::{CloneGetters, Getters};
use kubera_core::config::gateway::serde::read_configuration;
use kubera_core::config::gateway::types::GatewayConfiguration;
use kubera_core::continue_on;
use kubera_core::io::file_watcher::spawn_file_watcher;
use kubera_core::sync::signal::{Receiver, channel};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tokio::fs::read;
use tracing::info;

pub fn spawn_controller<P: AsRef<Path> + Default + Clone>(
    parameters: ConfigurationReaderParameters<P>,
) -> Result<Receiver<Option<GatewayConfiguration>>> {
    let (tx, rx) = channel(None);

    let file_watcher = spawn_file_watcher(&parameters.config_path())?;
    let config_path: PathBuf = parameters.config_path().as_ref().to_owned();

    tokio::spawn(async move {
        loop {
            if let Ok(config_reader) = read(&config_path).await.map(Cursor::new) {
                if let Ok(config) = read_configuration(config_reader) {
                    info!("Configuration file changed");
                    tx.replace(Some(config));
                }
            }
            continue_on!(file_watcher.changed());
        }
    });

    Ok(rx)
}

#[derive(Default, Debug, Builder, Clone, Getters, CloneGetters)]
#[builder(setter(into))]
pub struct ConfigurationReaderParameters<P: AsRef<Path> + Default + Clone> {
    #[getset(get_clone = "pub")]
    config_path: P,

    #[getset(get = "pub")]
    gateway_name: String,

    #[getset(get = "pub")]
    gateway_namespace: String,
}

impl<P: AsRef<Path> + Default + Clone> ConfigurationReaderParameters<P> {
    pub fn new_builder() -> ConfigurationReaderParametersBuilder<P> {
        Default::default()
    }
}
