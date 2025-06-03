use crate::http::QueryParamsExtension;
use http::request::Parts;
use http::{HeaderName, HeaderValue, Method};
use regex::Regex;
use unicase::UniCase;

pub trait Matcher<T> {
    fn matches(&self, part: &T) -> bool;
}

pub type CaseInsensitiveString = UniCase<String>;

pub enum HostnameMatcher {
    Exact(CaseInsensitiveString),
    Suffix(CaseInsensitiveString),
}

impl Matcher<HeaderValue> for HostnameMatcher {
    fn matches(&self, part: &HeaderValue) -> bool {
        match part.to_str().map(CaseInsensitiveString::from) {
            Ok(host) => match self {
                HostnameMatcher::Exact(expected) => expected == &host,
                HostnameMatcher::Suffix(expected) => host.ends_with(expected.as_str()),
            },
            Err(_) => false, // If the header value is not a valid UTF-8 string, it doesn't match
        }
    }
}

pub enum HeaderValueMatcher {
    Exact(String),
    RegularExpression(Regex),
}

impl Matcher<HeaderValue> for HeaderValueMatcher {
    fn matches(&self, value: &HeaderValue) -> bool {
        match self {
            HeaderValueMatcher::Exact(expected_value) => {
                value.to_str().map_or(false, |v| v == expected_value)
            }
            HeaderValueMatcher::RegularExpression(regex) => {
                value.to_str().map_or(false, |v| regex.is_match(v))
            }
        }
    }
}

pub struct HeaderNameMatcher(HeaderName);

impl Matcher<HeaderName> for HeaderNameMatcher {
    fn matches(&self, part: &HeaderName) -> bool {
        &self.0 == part
    }
}

pub struct HeaderMatcher {
    name: HeaderNameMatcher,
    value: HeaderValueMatcher,
}

impl Matcher<(&HeaderName, &HeaderValue)> for HeaderMatcher {
    fn matches(&self, (name, value): &(&HeaderName, &HeaderValue)) -> bool {
        self.name.matches(name) && self.value.matches(value)
    }
}

pub struct MethodMatcher(Method);

impl Matcher<Method> for MethodMatcher {
    fn matches(&self, method: &Method) -> bool {
        &self.0 == method
    }
}

pub enum PathMatcher {
    Exact(String),
    PathPrefix(String),
    RegularExpression(Regex),
}

impl Matcher<&str> for PathMatcher {
    fn matches(&self, path: &&str) -> bool {
        match self {
            PathMatcher::Exact(expected_path) => expected_path == path,
            PathMatcher::PathPrefix(prefix) => path.starts_with(prefix),
            PathMatcher::RegularExpression(regex) => regex.is_match(path),
        }
    }
}

pub enum QueryParamValueMatcher {
    Exact(String),
    RegularExpression(Regex),
}

impl Matcher<&str> for QueryParamValueMatcher {
    fn matches(&self, value: &&str) -> bool {
        match self {
            QueryParamValueMatcher::Exact(expected_value) => expected_value == value,
            QueryParamValueMatcher::RegularExpression(regex) => regex.is_match(value),
        }
    }
}

pub struct QueryParamNameMatcher(String);

impl Matcher<&str> for QueryParamNameMatcher {
    fn matches(&self, name: &&str) -> bool {
        self.0 == *name
    }
}

pub struct QueryParamMatcher {
    name: QueryParamNameMatcher,
    value: QueryParamValueMatcher,
}

impl Matcher<(&str, &str)> for QueryParamMatcher {
    fn matches(&self, (name, value): &(&str, &str)) -> bool {
        self.name.matches(name) && self.value.matches(value)
    }
}

pub struct RouteMatcher {
    hostnames: Vec<HostnameMatcher>,
    headers: Vec<HeaderMatcher>,
    method: Option<MethodMatcher>,
    path: Option<PathMatcher>,
    query_params: Vec<QueryParamMatcher>,
}

impl Matcher<Parts> for RouteMatcher {
    fn matches(&self, part: &Parts) -> bool {
        if !self.hostnames.is_empty() {
            if let Some(host) = part.headers.get(http_constant::HOST) {
                if !self.hostnames.iter().all(|matcher| matcher.matches(host)) {
                    return false;
                }
            } else {
                return false; // Required host header not found
            }
        }

        if !self.headers.is_empty() {
            for header in &self.headers {
                if let Some(value) = part.headers.get(&header.name.0) {
                    if !header.matches(&(&header.name.0, &value)) {
                        return false;
                    }
                } else {
                    return false; // Required header not found
                }
            }
        }

        if let Some(method) = &self.method {
            if !method.matches(&part.method) {
                return false;
            }
        }

        if let Some(path) = &self.path {
            if !path.matches(&part.uri.path()) {
                return false;
            }
        }

        if !self.query_params.is_empty() {
            let empty = vec![];
            let request_query_params = part
                .extensions
                .get::<QueryParamsExtension>()
                .map(|ext| ext.query_params())
                .unwrap_or(&empty);

            for param in &self.query_params {
                if !request_query_params
                    .iter()
                    .any(|(name, value)| param.matches(&(name, value)))
                {
                    return false; // Required query parameter not found
                }
            }
        }

        true
    }
}
