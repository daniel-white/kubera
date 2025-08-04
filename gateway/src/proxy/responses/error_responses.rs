use bytes::Bytes;
use http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use http::{Response, StatusCode};
use kubera_core::config::gateway::types::net::{ErrorResponseKind, ErrorResponses};
use kubera_core::config::gateway::types::GatewayConfiguration;
use kubera_core::continue_on;
use kubera_core::sync::signal::{signal, Receiver};
use kubera_macros::await_ready;
use problemdetails::Problem;
use std::borrow::Cow;
use strum::IntoStaticStr;
use tokio::task::JoinSet;
use url::Url;

pub fn error_responses(
    join_set: &mut JoinSet<()>,
    gateway_configuration_rx: &Receiver<GatewayConfiguration>,
) -> Receiver<ErrorResponseGenerators> {
    let (tx, rx) = signal::<ErrorResponseGenerators>();

    let gateway_configuration_rx = gateway_configuration_rx.clone();

    join_set.spawn(async move {
        loop {
            await_ready!(gateway_configuration_rx)
                .and_then(async |gateway_configuration| {
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
                            let authority = problem_detail
                                .authority()
                                .clone()
                                .unwrap_or_else(|| "http://kubera.whitefamily.in/problems/".into());
                            let authority = Url::parse(&authority).unwrap_or_else(|_| {
                                Url::parse("http://kubera.whitefamily.in/problems/").unwrap()
                            });
                            ErrorResponseGenerators::ProblemDetail(
                                ProblemDetailErrorResponseGenerator::new(authority),
                            )
                        }
                    };

                    tx.set(generator).await;
                })
                .run()
                .await;

            continue_on!(gateway_configuration_rx.changed())
        }
    });

    rx
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoStaticStr, Hash)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorResponseCode {
    NoRoute,
    MissingConfiguration,
    UpstreamUnavailable,
}

impl From<ErrorResponseCode> for StatusCode {
    fn from(code: ErrorResponseCode) -> Self {
        match code {
            ErrorResponseCode::NoRoute => StatusCode::NOT_FOUND,
            ErrorResponseCode::MissingConfiguration => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorResponseCode::UpstreamUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}

impl From<ErrorResponseCode> for Cow<'static, str> {
    fn from(code: ErrorResponseCode) -> Self {
        match code {
            ErrorResponseCode::NoRoute => "No matching route found".into(),
            ErrorResponseCode::MissingConfiguration => "Missing configuration".into(),
            ErrorResponseCode::UpstreamUnavailable => "Upstream unavailable".into(),
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

    fn body(&self, code: ErrorResponseCode) -> Option<(&'static str, Cow<'static, str>)> {
        None
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
struct EmptyErrorResponseGenerator;

impl ErrorResponseGenerator for EmptyErrorResponseGenerator {}

#[derive(PartialEq, Eq, Clone)]
struct HtmlErrorResponseGenerator;

impl ErrorResponseGenerator for HtmlErrorResponseGenerator {
    fn body(&self, code: ErrorResponseCode) -> Option<(&'static str, Cow<'static, str>)> {
        let message: Cow<_> = code.into();
        let body = format!("<html><body><h1>{message}</h1></body></html>");
        Some(("text/html", body.into()))
    }
}

#[derive(PartialEq, Eq, Clone)]
struct ProblemDetailErrorResponseGenerator {
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

        let body: String = serde_json::to_string(&problem.body).unwrap();

        Some(("application/problem+json", body.into()))
    }
}
