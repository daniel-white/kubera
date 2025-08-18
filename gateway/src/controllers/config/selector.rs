use getset::Getters;
use std::time::Instant;
use tracing::debug;
use typed_builder::TypedBuilder;
use vg_core::config::gateway::types::GatewayConfiguration;
use vg_core::continue_on;
use vg_core::sync::signal::{Receiver, signal};
use vg_core::task::Builder as TaskBuilder;

#[derive(Getters, Debug, Clone, TypedBuilder)]
pub struct SelectorParams {
    ipc_configuration_source_rx: Receiver<(Instant, GatewayConfiguration)>,
    fs_configuration_source_rx: Receiver<(Instant, GatewayConfiguration)>,
}

pub fn select_configuration(
    task_builder: &TaskBuilder,
    params: SelectorParams,
) -> Receiver<GatewayConfiguration> {
    let (tx, rx) = signal("selected_configuration");

    let ipc_config_source_rx = params.ipc_configuration_source_rx.clone();
    let fs_config_source_rx = params.fs_configuration_source_rx.clone();

    task_builder
        .new_task(stringify!(select_configuration))
        .spawn(async move {
            loop {
                let config = match (
                    ipc_config_source_rx.get().await,
                    fs_config_source_rx.get().await,
                ) {
                    (None, None) => {
                        debug!("No configuration available from either source");
                        None
                    }
                    (Some((_, ipc_config)), None) => {
                        debug!("Using IPC configuration");
                        Some(ipc_config)
                    }
                    (None, Some((_, fs_config))) => {
                        debug!("Using file-based configuration");
                        Some(fs_config)
                    }
                    (Some((ipc_serial, ipc_config)), Some((fs_serial, _)))
                        if fs_serial < ipc_serial =>
                    {
                        debug!("Using IPC configuration, newer");
                        Some(ipc_config)
                    }
                    (_, Some((_, fs_config))) => {
                        debug!("Using file-based configuration, newer");
                        Some(fs_config)
                    }
                };

                tx.replace(config).await;

                continue_on!(
                    params.ipc_configuration_source_rx.changed(),
                    params.fs_configuration_source_rx.changed()
                );
            }
        });

    rx
}
