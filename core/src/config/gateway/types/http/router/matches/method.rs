use http::Method;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use strum::EnumString;

#[derive(
    Validate,
    Default,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema,
    EnumString,
    Hash,
)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethodMatch {
    #[default]
    #[strum(serialize = "GET")]
    Get,
    #[strum(serialize = "POST")]
    Post,
    #[strum(serialize = "PUT")]
    Put,
    #[strum(serialize = "PATCH")]
    Patch,
    #[strum(serialize = "DELETE")]
    Delete,
    #[strum(serialize = "HEAD")]
    Head,
    #[strum(serialize = "OPTIONS")]
    Options,
    #[strum(serialize = "TRACE")]
    Trace,
    #[strum(serialize = "CONNECT")]
    Connect,
}

impl Into<Method> for HttpMethodMatch {
    fn into(self) -> Method {
        match self {
            Self::Get => Method::GET,
            Self::Post => Method::POST,
            Self::Put => Method::PUT,
            Self::Patch => Method::PATCH,
            Self::Delete => Method::DELETE,
            Self::Head => Method::HEAD,
            Self::Options => Method::OPTIONS,
            Self::Trace => Method::TRACE,
            Self::Connect => Method::CONNECT,
        }
    }
}
