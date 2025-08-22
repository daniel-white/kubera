use bytes::Bytes;
use http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use http::{Response, StatusCode};
use opentelemetry::trace::TraceContextExt;
use opentelemetry::TraceId;
use problemdetails::Problem;
use std::borrow::Cow;
use strum::IntoStaticStr;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use url::Url;
use vg_core::config::gateway::types::net::ErrorResponseKind;
use vg_core::config::gateway::types::GatewayConfiguration;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

pub fn error_responses(
    task_builder: &TaskBuilder,
    gateway_configuration_rx: &Receiver<GatewayConfiguration>,
) -> Receiver<ErrorResponseGenerators> {
    let (tx, rx) = signal("error_responses");

    let gateway_configuration_rx = gateway_configuration_rx.clone();

    task_builder
        .new_task(stringify!(error_responses))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(gateway_configuration) =
                    await_ready!(gateway_configuration_rx)
                {
                    let error_responses = gateway_configuration
                        .error_responses()
                        .clone()
                        .unwrap_or_default();

                    let generator = match error_responses.kind() {
                        ErrorResponseKind::Empty => {
                            ErrorResponseGenerators::Empty(EmptyErrorResponseGenerator)
                        }
                        ErrorResponseKind::Html => {
                            ErrorResponseGenerators::Html(HtmlErrorResponseGenerator)
                        }
                        ErrorResponseKind::ProblemDetail => {
                            let problem_detail =
                                error_responses.problem_detail().clone().unwrap_or_default();
                            let authority =
                                problem_detail.authority().clone().unwrap_or_else(|| {
                                    "http://vale-gateway.whitefamily.in/problems/".into()
                                });
                            let authority = Url::parse(&authority).unwrap_or_else(|_| {
                                Url::parse("http://vale-gateway.whitefamily.in/problems/").unwrap()
                            });
                            ErrorResponseGenerators::ProblemDetail(
                                ProblemDetailErrorResponseGenerator::new(authority),
                            )
                        }
                    };
                    tx.set(generator).await;
                }
                continue_on!(gateway_configuration_rx.changed());
            }
        });

    rx
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoStaticStr, Hash)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorResponseCode {
    NoRoute,
    AccessDenied,
    MissingConfiguration,
    UpstreamUnavailable,
    InvalidConfiguration,
}

impl From<ErrorResponseCode> for StatusCode {
    fn from(code: ErrorResponseCode) -> Self {
        match code {
            ErrorResponseCode::NoRoute => StatusCode::NOT_FOUND,
            ErrorResponseCode::AccessDenied => StatusCode::FORBIDDEN,
            ErrorResponseCode::MissingConfiguration => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorResponseCode::UpstreamUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            ErrorResponseCode::InvalidConfiguration => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<ErrorResponseCode> for Cow<'static, str> {
    fn from(code: ErrorResponseCode) -> Self {
        match code {
            ErrorResponseCode::NoRoute => "No matching route found".into(),
            ErrorResponseCode::AccessDenied => "Access denied".into(),
            ErrorResponseCode::MissingConfiguration => "Missing configuration".into(),
            ErrorResponseCode::UpstreamUnavailable => "Upstream unavailable".into(),
            ErrorResponseCode::InvalidConfiguration => "Invalid configuration".into(),
        }
    }
}

trait ErrorResponseGenerator: PartialEq {
    fn get_response(&self, code: ErrorResponseCode) -> Response<Option<Bytes>> {
        let status_code: StatusCode = code.into();
        let body = self.body(code);

        let response = Response::builder().status(status_code);

        match body {
            Some((content_type, body_content)) => {
                let body = Bytes::from(body_content.into_owned());
                response
                    .header(CONTENT_TYPE, content_type)
                    .header(CONTENT_LENGTH, body.len())
                    .body(Some(body))
                    .expect("Failed to build error response")
            }
            None => response.body(None).expect("Failed to build error response"),
        }
    }

    fn body(&self, _code: ErrorResponseCode) -> Option<(&'static str, Cow<'static, str>)> {
        None
    }

    fn trace_id(&self) -> Option<String> {
        let span = Span::current();
        let context = span.context();
        let trace_id = context.span().span_context().trace_id();
        if trace_id != TraceId::INVALID {
            Some(trace_id.to_string())
        } else {
            None
        }
    }
}

#[derive(PartialEq, Eq, Clone)]
pub enum ErrorResponseGenerators {
    Empty(EmptyErrorResponseGenerator),
    Html(HtmlErrorResponseGenerator),
    ProblemDetail(ProblemDetailErrorResponseGenerator),
}

impl ErrorResponseGenerators {
    pub fn get_response(&self, code: ErrorResponseCode) -> Response<Option<Bytes>> {
        match self {
            Self::Empty(generator) => generator.get_response(code),
            Self::Html(generator) => generator.get_response(code),
            Self::ProblemDetail(generator) => generator.get_response(code),
        }
    }
}

impl Default for ErrorResponseGenerators {
    fn default() -> Self {
        ErrorResponseGenerators::Empty(EmptyErrorResponseGenerator)
    }
}

#[derive(PartialEq, Eq, Clone)]
pub(crate) struct EmptyErrorResponseGenerator;

impl ErrorResponseGenerator for EmptyErrorResponseGenerator {}

#[derive(PartialEq, Eq, Clone)]
pub(crate) struct HtmlErrorResponseGenerator;

impl ErrorResponseGenerator for HtmlErrorResponseGenerator {
    fn body(&self, code: ErrorResponseCode) -> Option<(&'static str, Cow<'static, str>)> {
        let message: Cow<_> = code.into();
        let body = format!("<html><body><h1>{message}</h1></body></html>");
        Some(("text/html", body.into()))
    }
}

#[derive(PartialEq, Eq, Clone)]
pub(crate) struct ProblemDetailErrorResponseGenerator {
    authority: Url,
}

impl ProblemDetailErrorResponseGenerator {
    pub fn new(authority: Url) -> Self {
        Self { authority }
    }
}

impl ErrorResponseGenerator for ProblemDetailErrorResponseGenerator {
    fn body(&self, code: ErrorResponseCode) -> Option<(&'static str, Cow<'static, str>)> {
        let message: Cow<_> = code.into();
        let code_str: &'static str = code.into();
        let status_code: StatusCode = code.into();

        let problem = Problem::from(code)
            .with_value("status", status_code.as_u16())
            .with_type(format!("{}{}", self.authority, code_str))
            .with_detail(message);

        let problem = if let Some(trace_id) = self.trace_id() {
            problem.with_value("trace_id", trace_id)
        } else {
            problem
        };

        let body: String = serde_json::to_string(&problem.body).unwrap();

        Some(("application/problem+json", body.into()))
    }
}
