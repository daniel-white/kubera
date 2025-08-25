use vg_core::gateways::Gateway;
use vg_core::http::listeners::HttpListener;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

pub fn http_listener(
    task_builder: &TaskBuilder,
    gateway_rx: &Receiver<Gateway>,
) -> Receiver<Option<HttpListener>> {
    let (tx, rx) = signal(stringify!(http_listener));
    let gateway_rx = gateway_rx.clone();

    task_builder
        .new_task(stringify!(http_listener))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(gateway) = await_ready!(gateway_rx) {
                    let http_listener = gateway.http_listener().clone();
                    tx.set(http_listener).await;
                }
                continue_on!(gateway_rx.changed());
            }
        });

    rx
}
