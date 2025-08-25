use super::error_codes::ErrorResponseCode;
use bytes::Bytes;
use http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use http::{Response, StatusCode};
use opentelemetry::trace::TraceContextExt;
use opentelemetry::TraceId;
use problemdetails::Problem;
use std::borrow::Cow;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use typed_builder::TypedBuilder;
use url::Url;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum HttpErrorResponseGeneratorType {
    Empty(EmptyErrorResponseGenerator),
    Html(HtmlErrorResponseGenerator),
    ProblemDetail(ProblemDetailErrorResponseGenerator),
}

impl HttpErrorResponseGeneratorType {
    pub fn generate_response(&self, code: ErrorResponseCode) -> Response<Option<Bytes>> {
        match self {
            Self::Empty(generator) => generator.get_response(code),
            Self::Html(generator) => generator.get_response(code),
            Self::ProblemDetail(generator) => generator.get_response(code),
        }
    }
}

trait HttpErrorResponseGenerator: PartialEq {
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

#[derive(Debug, PartialEq, Eq, Clone, TypedBuilder)]
pub struct EmptyErrorResponseGenerator {}

impl HttpErrorResponseGenerator for EmptyErrorResponseGenerator {}

#[derive(Debug, PartialEq, Eq, Clone, TypedBuilder)]
pub struct HtmlErrorResponseGenerator {}

impl HttpErrorResponseGenerator for HtmlErrorResponseGenerator {
    fn body(&self, code: ErrorResponseCode) -> Option<(&'static str, Cow<'static, str>)> {
        let message: Cow<_> = code.into();
        let body = format!("<html><body><h1>{message}</h1></body></html>");
        Some(("text/html", body.into()))
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ProblemDetailErrorResponseGenerator {
    authority: Option<Url>,
}

impl ProblemDetailErrorResponseGenerator {
    pub fn builder() -> ProblemDetailErrorResponseGeneratorBuilder {
        ProblemDetailErrorResponseGeneratorBuilder { authority: None }
    }
}

impl HttpErrorResponseGenerator for ProblemDetailErrorResponseGenerator {
    fn body(&self, code: ErrorResponseCode) -> Option<(&'static str, Cow<'static, str>)> {
        let message: Cow<_> = code.into();
        let code_str: &'static str = code.into();
        let status_code: StatusCode = code.into();

        let problem = Problem::from(code)
            .with_value("status", status_code.as_u16())
            .with_type(format!(
                "{}{}",
                self.authority
                    .as_ref()
                    .map(|a| a.as_str())
                    .unwrap_or("http://vale-gateway.whitefamily.in/errors/"),
                code_str
            ))
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

pub struct ProblemDetailErrorResponseGeneratorBuilder {
    authority: Option<Url>,
}

impl ProblemDetailErrorResponseGeneratorBuilder {
    pub fn build(self) -> ProblemDetailErrorResponseGenerator {
        ProblemDetailErrorResponseGenerator {
            authority: self.authority,
        }
    }

    pub fn authority<A: Into<Url>>(&mut self, authority: A) -> &mut Self {
        self.authority = Some(authority.into());
        self
    }
}
