use getset::Getters;
use std::io::Cursor;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::fs::read;
use tracing::{debug, info};
use typed_builder::TypedBuilder;
use vg_core::continue_after;
use vg_core::gateways::Gateway;
use vg_core::io::file_watcher::spawn_file_watcher;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;

#[derive(Debug, Getters, TypedBuilder)]
pub struct FileSourceParams {
    #[builder(setter(into))]
    file_path: PathBuf,
}

pub fn file_source(
    task_builder: &TaskBuilder,
    params: FileSourceParams,
) -> Receiver<(Instant, Gateway)> {
    let (tx, rx) = signal(stringify!(watch_configuration_file));

    task_builder
        .new_task(stringify!(watch_configuration_file))
        .spawn(async move {
            info!(
                "Spawning file watcher for configuration file: {:?}",
                params.file_path
            );

            let file_watcher =
                spawn_file_watcher(&params.file_path).expect("Failed to spawn file watcher");

            loop {
                let serial = Instant::now();

                if let Ok(reader) = read(&params.file_path).await.map(Cursor::new)
                    && let Ok(gateway) = serde_yaml::from_reader(reader)
                {
                    debug!("Configuration file read");
                    tx.set((serial, gateway)).await;
                }

                continue_after!(
                    Duration::from_secs(30), // failsafe timeout to force a re-read
                    file_watcher.changed()
                );
            }
        });

    rx
}
