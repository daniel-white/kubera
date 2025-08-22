use super::AccessControlFilterHandler;
use std::collections::HashMap;
use vg_core::config::gateway::types::net::AccessControlFilter;
use vg_core::config::gateway::types::GatewayConfiguration;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::types::filters::access_control::Key;
use vg_core::{await_ready, continue_on, ReadyState};

pub type AccessControlFilterHandlers = HashMap<Key, AccessControlFilterHandler>;

fn access_control_filters_config(
    task_builder: &TaskBuilder,
    gateway_configuration_rx: &Receiver<GatewayConfiguration>,
) -> Receiver<HashMap<Key, AccessControlFilter>> {
    let (tx, rx) = signal(stringify!(access_control_filters));
    let gateway_configuration_rx = gateway_configuration_rx.clone();

    task_builder
        .new_task(stringify!(access_control_filters))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(gateway_configuration) =
                    await_ready!(gateway_configuration_rx)
                {
                    let filters: HashMap<Key, AccessControlFilter> = gateway_configuration
                        .access_control_filters()
                        .iter()
                        .map(|f| (f.key().clone(), f.clone()))
                        .collect();
                    tx.set(filters).await;
                }
                continue_on!(gateway_configuration_rx.changed());
            }
        });

    rx
}

pub fn access_control_filters_handlers(
    task_builder: &TaskBuilder,
    gateway_configuration_rx: &Receiver<GatewayConfiguration>,
) -> Receiver<AccessControlFilterHandlers> {
    let (tx, rx) = signal(stringify!(access_control_filters_handlers));
    let config_rx = access_control_filters_config(task_builder, gateway_configuration_rx);

    task_builder
        .new_task(stringify!(access_control_filters_handlers))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(config) = await_ready!(config_rx) {
                    let handlers = config
                        .iter()
                        .map(|(key, filter)| {
                            (key.clone(), AccessControlFilterHandler::builder().build())
                        })
                        .collect();
                    tx.set(handlers).await;
                }
                continue_on!(config_rx.changed());
            }
        });
    rx
}
