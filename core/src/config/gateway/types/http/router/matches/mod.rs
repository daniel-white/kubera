mod headers;
mod host_header;
mod method;
mod path;
mod query_params;

pub use self::headers::*;
pub use self::host_header::*;
pub use self::method::*;
pub use self::path::*;
pub use self::query_params::*;
use derive_builder::Builder;
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpRouteMatches {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    path: Option<HttpPathMatch>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(max_items = 16)]
    headers: Option<Vec<HttpHeaderMatch>>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(max_items = 16)]
    query_params: Option<Vec<HttpQueryParamMatch>>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    method: Option<HttpMethodMatch>,
}
