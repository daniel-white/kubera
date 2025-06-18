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
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpRouteRuleMatches {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "HttpPathMatch::is_default")]
    path: HttpPathMatch,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(max_items = 16)]
    headers: Option<Vec<HttpHeaderMatch>>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(max_items = 16)]
    query_params: Option<Vec<HttpQueryParamMatch>>,

    #[getset(get = "pub")]
    #[serde(default)]
    method: HttpMethodMatch,
}

#[derive(Debug, Default)]
pub struct HttpRouteRuleMatchesBuilder {
    path: HttpPathMatch,
    headers: Option<Vec<HttpHeaderMatch>>,
    query_params: Option<Vec<HttpQueryParamMatch>>,
    method: HttpMethodMatch,
}

impl HttpRouteRuleMatchesBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(self) -> HttpRouteRuleMatches {
        HttpRouteRuleMatches {
            path: self.path,
            headers: self.headers,
            query_params: self.query_params,
            method: self.method,
        }
    }
    pub fn with_exact_path<S: AsRef<str>>(&mut self, path: S) -> &mut Self {
        self.path = HttpPathMatch::exactly(path);
        self
    }

    pub fn with_path_prefix<S: AsRef<str>>(&mut self, prefix: S) -> &mut Self {
        self.path = HttpPathMatch::with_prefix(prefix);
        self
    }

    pub fn with_path_matching<S: AsRef<str>>(&mut self, pattern: S) -> &mut Self {
        self.path = HttpPathMatch::matching(pattern);
        self
    }

    pub fn with_method(&mut self, method: HttpMethodMatch) -> &mut Self {
        self.method = method;
        self
    }

    pub fn add_exact_header<N: AsRef<str>, V: AsRef<str>>(
        &mut self,
        name: N,
        value: V,
    ) -> &mut Self {
        let header_match = HttpHeaderMatch::exactly(name, value);
        self.headers.get_or_insert_default().push(header_match);
        self
    }

    pub fn add_header_matching<N: AsRef<str>, P: AsRef<str>>(
        &mut self,
        name: N,
        pattern: P,
    ) -> &mut Self {
        let header_match = HttpHeaderMatch::matches(name, pattern);
        self.headers.get_or_insert_default().push(header_match);
        self
    }

    pub fn add_exact_query_param<N: AsRef<str>, V: AsRef<str>>(
        &mut self,
        name: N,
        value: V,
    ) -> &mut Self {
        let query_param_match = HttpQueryParamMatch::exactly(name, value);
        self.query_params
            .get_or_insert_default()
            .push(query_param_match);
        self
    }

    pub fn add_query_param_matching<N: AsRef<str>, P: AsRef<str>>(
        &mut self,
        name: N,
        pattern: P,
    ) -> &mut Self {
        let query_param_match = HttpQueryParamMatch::matches(name, pattern);
        self.query_params
            .get_or_insert_default()
            .push(query_param_match);
        self
    }
}
