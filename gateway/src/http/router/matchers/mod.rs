mod headers;
mod host_header;
mod method;
mod path;
mod query_params;
mod score;

use crate::http::router::matchers::MatchResult::{Matched, NotMatched};
use crate::http::router::matchers::headers::HeaderMatcher;
use crate::http::router::matchers::query_params::QueryParamMatcher;
use headers::HeadersMatcher;
use host_header::{HostHeaderMatcher, HostHeaderValueMatcher};
use http::request::Parts;
use http::{HeaderName, HeaderValue};
use method::MethodMatcher;
use path::PathMatcher;
use query_params::QueryParamsMatcher;
use score::Score;
use std::borrow::Cow;
use unicase::UniCase;

pub(self) type CaseInsensitiveString = UniCase<String>;

pub(self) trait Matcher<T> {
    fn matches(&self, score: &Score, part: &T) -> bool;
}

#[derive(Debug, PartialEq, Clone)]
pub struct RouteMatcher {
    host_header: Option<HostHeaderMatcher>,
    path: Option<PathMatcher>,
    method: Option<MethodMatcher>,
    headers: Option<HeadersMatcher>,
    query_params: Option<QueryParamsMatcher>,
}

#[derive(Default)]
pub struct RouteMatcherBuilder {
    host_header: Option<HostHeaderMatcher>,
    path: Option<PathMatcher>,
    method: Option<MethodMatcher>,
    headers: Option<HeadersMatcher>,
    query_params: Option<QueryParamsMatcher>,
}

pub enum MatchResult {
    Matched(Score),
    NotMatched,
}

impl RouteMatcher {
    pub fn new_builder() -> RouteMatcherBuilder {
        RouteMatcherBuilder::default()
    }

    pub fn matches(&self, parts: &Parts) -> MatchResult {
        let score = Score::default();

        if let Some(host_header_matcher) = &self.host_header {
            if !host_header_matcher.matches(&score, &parts.headers) {
                return NotMatched;
            }
        }

        if let Some(path_matcher) = &self.path {
            if !path_matcher.matches(&score, &parts.uri.path()) {
                return NotMatched;
            }
        }

        if let Some(method_matcher) = &self.method {
            if !method_matcher.matches(&score, &parts.method) {
                return NotMatched;
            }
        }

        if let Some(headers_matcher) = &self.headers {
            if !headers_matcher.matches(&score, &parts.headers) {
                return NotMatched;
            }
        }

        if let Some(query_params_matcher) = &self.query_params {
            let query_params: Vec<(Cow<str>, Cow<str>)> = parts
                .uri
                .query()
                .map(|query| url::form_urlencoded::parse(query.as_bytes()).collect())
                .unwrap_or_else(Vec::new);
            if !query_params_matcher.matches(&score, &query_params) {
                return NotMatched;
            }
        }

        Matched(score)
    }
}

impl RouteMatcherBuilder {
    pub fn build(self) -> RouteMatcher {
        RouteMatcher {
            host_header: self.host_header,
            path: self.path,
            method: self.method,
            headers: self.headers,
            query_params: self.query_params,
        }
    }

    pub fn with_host(&mut self, host: &str) -> &mut Self {
        self.host_header
            .get_or_insert_default()
            .matchers
            .push(HostHeaderValueMatcher::Exact(CaseInsensitiveString::from(
                host,
            )));
        self
    }

    pub fn with_host_suffix(&mut self, suffix: &str) -> &mut Self {
        self.host_header
            .get_or_insert_default()
            .matchers
            .push(HostHeaderValueMatcher::Suffix(CaseInsensitiveString::from(
                suffix,
            )));
        self
    }

    pub fn with_exact_path(&mut self, path: &str) -> &mut Self {
        self.path = Some(PathMatcher::Exact(path.to_string()));
        self
    }

    pub fn with_path_prefix(&mut self, prefix: &str) -> &mut Self {
        self.path = Some(PathMatcher::Prefix(prefix.to_string()));
        self
    }

    pub fn with_path_matching(&mut self, pattern: &str) -> &mut Self {
        self.path = Some(PathMatcher::RegularExpression(pattern.to_string()));
        self
    }

    pub fn with_method(&mut self, method: http::Method) -> &mut Self {
        self.method.get_or_insert_default().methods.insert(method);
        self
    }

    pub fn with_header(&mut self, name: &HeaderName, value: &HeaderValue) -> &mut Self {
        self.headers
            .get_or_insert_default()
            .matchers
            .push(HeaderMatcher::new(name, value));
        self
    }

    pub fn with_header_matching(&mut self, name: &HeaderName, pattern: &str) -> &mut Self {
        self.headers
            .get_or_insert_default()
            .matchers
            .push(HeaderMatcher::new_matching(name, pattern));
        self
    }

    pub fn with_query_param(&mut self, name: &str, value: &str) -> &mut Self {
        self.query_params
            .get_or_insert_default()
            .matchers
            .push(QueryParamMatcher::new(name, value));
        self
    }

    pub fn with_query_param_matching(&mut self, name: &str, pattern: &str) -> &mut Self {
        self.query_params
            .get_or_insert_default()
            .matchers
            .push(QueryParamMatcher::new_matching(name, pattern));
        self
    }
}
