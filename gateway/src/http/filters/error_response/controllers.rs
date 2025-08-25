use crate::http::filters::error_response::handler::HttpErrorResponseFilterHandler;
use std::collections::HashMap;
use vg_core::http::filters::error_response::{
    HttpErrorResponseFilter, HttpErrorResponseFilterKey, HttpErrorResponseKind,
};
use vg_core::http::listeners::{HttpFilterDefinition, HttpListener};
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

fn http_error_responses_filters(
    task_builder: &TaskBuilder,
    http_listener_rx: &Receiver<Option<HttpListener>>,
) -> Receiver<HashMap<HttpErrorResponseFilterKey, HttpErrorResponseFilter>> {
    let (tx, rx) = signal(stringify!(http_error_responses_filters));
    let http_listener_rx = http_listener_rx.clone();

    task_builder
        .new_task(stringify!(http_error_responses_filters))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(http_listener) = await_ready!(http_listener_rx) {
                    let filters = match http_listener.as_ref() {
                        Some(http_listener) => http_listener
                            .filter_definitions()
                            .iter()
                            .filter_map(|f| match f {
                                HttpFilterDefinition::ErrorResponse(filter) => {
                                    Some((filter.key().clone(), filter.clone()))
                                }
                                _ => None,
                            })
                            .collect(),
                        None => HashMap::new(),
                    };

                    tx.set(filters).await;
                }
                continue_on!(http_listener_rx.changed());
            }
        });

    rx
}

pub fn http_error_response_filter_handlers(
    task_builder: &TaskBuilder,
    http_listener: &Receiver<Option<HttpListener>>,
) -> Receiver<HashMap<HttpErrorResponseFilterKey, HttpErrorResponseFilterHandler>> {
    let (tx, rx) = signal(stringify!(error_response_filter_handlers));
    let filters_rx = http_error_responses_filters(task_builder, http_listener);

    task_builder
        .new_task(stringify!(error_responses))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(filters) = await_ready!(filters_rx) {
                    let handlers = filters
                        .iter()
                        .map(|(key, filter)| {
                            let mut builder = HttpErrorResponseFilterHandler::builder();
                            let handler = match filter.kind() {
                                HttpErrorResponseKind::Empty => {
                                    builder.empty_responses();
                                    builder.build()
                                }
                                HttpErrorResponseKind::Html => {
                                    builder.html_responses();
                                    builder.build()
                                }
                                HttpErrorResponseKind::ProblemDetail => {
                                    builder.problem_detail_responses(|builder| {
                                        if let Some(authority) = filter
                                            .problem_detail()
                                            .as_ref()
                                            .and_then(|pd| pd.authority().clone())
                                        {
                                            builder.authority(authority);
                                        }
                                    });
                                    builder.build()
                                }
                            };

                            (key.clone(), handler)
                        })
                        .collect();
                    tx.set(handlers).await;
                }
                continue_on!(filters_rx.changed());
            }
        });

    rx
}
