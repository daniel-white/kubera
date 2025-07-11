mod headers;
mod host;
mod host_header;
mod method;
mod path;
mod query_params;
mod score;

use headers::*;
pub use host::*;
pub use host_header::*;
use http::request::Parts;
use http::{HeaderName, HeaderValue};
use method::*;
use path::*;
use query_params::*;
pub use score::HttpRouteRuleMatchesScore;
use std::borrow::Cow;
use tracing::{debug, instrument, trace};

trait Match<T> {
    fn matches(&self, score: &HttpRouteRuleMatchesScore, part: &T) -> bool;
}

#[derive(Debug, PartialEq, Clone)]
pub struct HttpRouteRuleMatches {
    path: Option<PathMatch>,
    method: Option<MethodMatch>,
    headers: Option<HeadersMatch>,
    query_params: Option<QueryParamsMatch>,
}

pub enum HttpRouteRuleMatchesResult {
    Matched(HttpRouteRuleMatchesScore),
    NotMatched,
}

impl HttpRouteRuleMatches {
    #[instrument(
        skip(self, parts),
        level = "trace",
        name = "HttpRouteRuleMatches::matches"
    )]
    pub fn matches(&self, parts: &Parts) -> HttpRouteRuleMatchesResult {
        use HttpRouteRuleMatchesResult::*;
        let score = HttpRouteRuleMatchesScore::default();

        if let Some(method_matcher) = &self.method {
            trace!("Testing method for match");
            if !method_matcher.matches(&score, &parts.method) {
                debug!("Method did not match");
                return NotMatched;
            }
        }

        if let Some(path_matcher) = &self.path {
            trace!("Testing path for match");
            if !path_matcher.matches(&score, &parts.uri.path()) {
                debug!("Path did not match");
                return NotMatched;
            }
        }

        if let Some(headers_matcher) = &self.headers {
            trace!("Testing headers for match");
            if !headers_matcher.matches(&score, &parts.headers) {
                debug!("Headers did not match");
                return NotMatched;
            }
        }

        if let Some(query_params_matcher) = &self.query_params {
            trace!("Testing query parameters for match");
            let query_params: Vec<(Cow<str>, Cow<str>)> = parts
                .uri
                .query()
                .map(|query| url::form_urlencoded::parse(query.as_bytes()).collect())
                .unwrap_or_default();
            if !query_params_matcher.matches(&score, &query_params) {
                debug!("Query parameters did not match");
                return NotMatched;
            }
        }

        Matched(score)
    }
}

#[derive(Default, Debug)]
pub struct HttpRouteRuleMatchesBuilder {
    path: Option<PathMatch>,
    method: Option<MethodMatch>,
    headers: Option<HeadersMatch>,
    query_params: Option<QueryParamsMatch>,
}

impl HttpRouteRuleMatchesBuilder {
    pub fn build(self) -> HttpRouteRuleMatches {
        HttpRouteRuleMatches {
            path: self.path,
            method: self.method,
            headers: self.headers,
            query_params: self.query_params,
        }
    }

    pub fn with_exact_path(&mut self, path: &str) -> &mut Self {
        self.path = Some(PathMatch::Exact(path.to_string()));
        self
    }

    pub fn with_path_prefix(&mut self, prefix: &str) -> &mut Self {
        self.path = Some(PathMatch::Prefix(prefix.to_string()));
        self
    }

    pub fn with_path_matching(&mut self, pattern: &str) -> &mut Self {
        self.path = Some(PathMatch::RegularExpression(pattern.to_string()));
        self
    }

    pub fn with_method(&mut self, method: http::Method) -> &mut Self {
        self.method.get_or_insert_default().method = method;
        self
    }

    pub fn with_exact_header(&mut self, name: HeaderName, value: HeaderValue) -> &mut Self {
        self.headers
            .get_or_insert_default()
            .header_matches
            .push(HeaderMatch::new_exact(name, value));
        self
    }

    pub fn with_header_matching(&mut self, name: HeaderName, pattern: &str) -> &mut Self {
        self.headers
            .get_or_insert_default()
            .header_matches
            .push(HeaderMatch::new_matching(name, pattern));
        self
    }

    pub fn with_exact_query_param(&mut self, name: &str, value: &str) -> &mut Self {
        self.query_params
            .get_or_insert_default()
            .query_param_matches
            .push(QueryParamMatch::new_exact(name, value));
        self
    }

    pub fn with_query_param_matching(&mut self, name: &str, pattern: &str) -> &mut Self {
        self.query_params
            .get_or_insert_default()
            .query_param_matches
            .push(QueryParamMatch::new_matching(name, pattern));
        self
    }
}
