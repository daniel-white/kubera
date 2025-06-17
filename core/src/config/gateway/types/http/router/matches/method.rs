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

impl From<HttpMethodMatch> for Method {
    fn from(method_match: HttpMethodMatch) -> Method {
        match method_match {
            HttpMethodMatch::Get => Method::GET,
            HttpMethodMatch::Post => Method::POST,
            HttpMethodMatch::Put => Method::PUT,
            HttpMethodMatch::Patch => Method::PATCH,
            HttpMethodMatch::Delete => Method::DELETE,
            HttpMethodMatch::Head => Method::HEAD,
            HttpMethodMatch::Options => Method::OPTIONS,
            HttpMethodMatch::Trace => Method::TRACE,
            HttpMethodMatch::Connect => Method::CONNECT,
        }
    }
}
