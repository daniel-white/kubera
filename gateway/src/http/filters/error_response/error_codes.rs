use http::StatusCode;
use std::borrow::Cow;
use strum::IntoStaticStr;

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
