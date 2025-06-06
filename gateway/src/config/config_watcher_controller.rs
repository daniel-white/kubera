use anyhow::Result;
use kubera_core::config::gateway::serde::read_configuration;
use kubera_core::config::gateway::types::GatewayConfiguration;
use kubera_core::io::file_watcher::spawn_file_watcher;
use kubera_core::select_continue;
use kubera_core::sync::signal::{channel, Receiver};
use std::io::Cursor;
use std::path::Path;
use tokio::fs::read;
use tracing::info;

pub struct ControllerError;

pub fn spawn_controller<P: AsRef<Path>>(
    config_path: P,
) -> Result<Receiver<Option<GatewayConfiguration>>> {
    let (tx, rx) = channel(None);

    let mut file_watcher = spawn_file_watcher(&config_path)?;
    let config_path = config_path.as_ref().to_owned();

    tokio::spawn(async move {
        loop {
            if let Ok(config_reader) = read(&config_path).await.map(Cursor::new) {
                if let Ok(config) = read_configuration(config_reader) {
                    info!("Configuration file changed");
                    tx.replace(Some(config));
                }
            }
            select_continue!(file_watcher.changed());
        }
    });

    Ok(rx)
}
