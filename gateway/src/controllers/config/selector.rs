use getset::Getters;
use std::time::Instant;
use tracing::debug;
use typed_builder::TypedBuilder;
use vg_core::continue_on;
use vg_core::gateways::Gateway;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;

#[derive(Getters, Debug, Clone, TypedBuilder)]
pub struct GatewayParams {
    ipc_source_rx: Receiver<(Instant, Gateway)>,
    fs_source_rx: Receiver<(Instant, Gateway)>,
}

pub fn gateway(task_builder: &TaskBuilder, params: GatewayParams) -> Receiver<Gateway> {
    let (tx, rx) = signal("gateway");

    let ipc_source_rx = params.ipc_source_rx.clone();
    let fs_source_rx = params.fs_source_rx.clone();

    task_builder
        .new_task(stringify!(select_configuration))
        .spawn(async move {
            loop {
                let gateway = match (
                    ipc_source_rx.get().await.as_ref(),
                    fs_source_rx.get().await.as_ref(),
                ) {
                    (Some((_, ipc)), None) => {
                        debug!("Using IPC configuration");
                        Some(ipc.clone())
                    }
                    (None, Some((_, fs))) => {
                        debug!("Using file-based configuration");
                        Some(fs_config.clone())
                    }
                    (Some((ipc_serial, ipc)), Some((fs_serial, _))) if fs_serial < ipc_serial => {
                        debug!("Using IPC configuration, newer");
                        Some(ipc.clone())
                    }
                    (_, Some((_, fs))) => {
                        debug!("Using file-based configuration, newer");
                        Some(fs.clone())
                    }
                    _ => {
                        debug!("No configuration available from either source");
                        None
                    }
                };

                tx.replace(gateway).await;

                continue_on!(
                    params.ipc_source_rx.changed(),
                    params.fs_source_rx.changed()
                );
            }
        });

    rx
}
