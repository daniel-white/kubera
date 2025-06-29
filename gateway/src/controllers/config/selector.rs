use derive_builder::Builder;
use getset::Getters;
use kubera_core::config::gateway::types::GatewayConfiguration;
use kubera_core::continue_on;
use kubera_core::sync::signal::{channel, Receiver};
use std::time::Instant;
use tokio::task::JoinSet;
use tracing::debug;

#[derive(Getters, Debug, Clone, Builder)]
pub struct SelectorParams {
    #[getset(get = "pub")]
    ipc_configuration_source: Receiver<Option<(Instant, GatewayConfiguration)>>,
    #[getset(get = "pub")]
    fs_configuration_source: Receiver<Option<(Instant, GatewayConfiguration)>>,
}

impl SelectorParams {
    pub fn new_builder() -> SelectorParamsBuilder {
        SelectorParamsBuilder::default()
    }
}

pub fn select_configuration(
    join_set: &mut JoinSet<()>,
    params: SelectorParams,
) -> Receiver<Option<GatewayConfiguration>> {
    let (tx, rx) = channel(None);

    let ipc_config_source = params.ipc_configuration_source.clone();
    let fs_config_source = params.fs_configuration_source.clone();

    join_set.spawn(async move {
        loop {
            let ipc_config = ipc_config_source.current();
            let fs_config = fs_config_source.current();

            let config = match (ipc_config.as_ref(), fs_config.as_ref()) {
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

            tx.replace(config.cloned());

            continue_on!(
                params.ipc_configuration_source().changed(),
                params.fs_configuration_source().changed()
            );
        }
    });

    rx
}
