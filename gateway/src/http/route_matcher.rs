use super::extensions::QueryParamsExtension;
use crate::util::get_regex;
use derive_builder::Builder;
use http::request::Parts;
use http::{HeaderName, HeaderValue, Method};
use unicase::UniCase;

pub trait Matcher<T> {
    fn matches(&self, part: &T) -> bool;
}

type CaseInsensitiveString = UniCase<String>;

#[derive(Debug, PartialEq, Clone)]
enum HostnameMatcher {
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

#[derive(Debug, PartialEq, Clone)]
enum HeaderValueMatcher {
    Exact(HeaderValue),
    RegularExpression(String),
}

impl Matcher<HeaderValue> for HeaderValueMatcher {
    fn matches(&self, value: &HeaderValue) -> bool {
        match self {
            HeaderValueMatcher::Exact(expected_value) => {
                value.to_str().map_or(false, |v| v == expected_value)
            }
            HeaderValueMatcher::RegularExpression(pattern) => {
                let regex = get_regex(pattern);
                value.to_str().map_or(false, |v| regex.is_match(v))
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
struct HeaderNameMatcher(HeaderName);

impl HeaderNameMatcher {
    pub fn new(name: HeaderName) -> Self {
        HeaderNameMatcher(name)
    }
}

impl Matcher<HeaderName> for HeaderNameMatcher {
    fn matches(&self, part: &HeaderName) -> bool {
        &self.0 == part
    }
}

#[derive(Builder, Debug, PartialEq, Clone)]
struct HeaderMatcher {
    name: HeaderNameMatcher,
    value: HeaderValueMatcher,
}

impl Matcher<(&HeaderName, &HeaderValue)> for HeaderMatcher {
    fn matches(&self, (name, value): &(&HeaderName, &HeaderValue)) -> bool {
        self.name.matches(name) && self.value.matches(value)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct MethodMatcher(Method);

impl Default for MethodMatcher {
    fn default() -> Self {
        MethodMatcher(Method::default())
    }
}

impl MethodMatcher {
    pub fn is_default(&self) -> bool {
        self.0 == Method::default()
    }
}

impl Matcher<Method> for MethodMatcher {
    fn matches(&self, method: &Method) -> bool {
        &self.0 == method
    }
}

#[derive(Debug, PartialEq, Clone)]
enum PathMatcher {
    Exact(String),
    Prefix(String),
    RegularExpression(String),
}

impl Default for PathMatcher {
    fn default() -> Self {
        PathMatcher::Prefix("/".to_string())
    }
}

impl PathMatcher {
    pub fn is_default(&self) -> bool {
        matches!(self, PathMatcher::Prefix(prefix) if prefix == "/")
    }
}

impl Matcher<&str> for PathMatcher {
    fn matches(&self, path: &&str) -> bool {
        match self {
            PathMatcher::Exact(expected_path) => expected_path == path,
            PathMatcher::Prefix(prefix) => path.starts_with(prefix),
            PathMatcher::RegularExpression(pattern) => {
                let regex = get_regex(pattern);
                regex.is_match(path)
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
enum QueryParamValueMatcher {
    Exact(String),
    RegularExpression(String),
}

impl Matcher<&str> for QueryParamValueMatcher {
    fn matches(&self, value: &&str) -> bool {
        match self {
            QueryParamValueMatcher::Exact(expected_value) => expected_value == value,
            QueryParamValueMatcher::RegularExpression(regex) => {
                let regex = get_regex(regex);
                regex.is_match(value)
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
struct QueryParamNameMatcher(String);

impl Matcher<&str> for QueryParamNameMatcher {
    fn matches(&self, name: &&str) -> bool {
        self.0 == *name
    }
}

#[derive(Debug, PartialEq, Clone)]
struct QueryParamMatcher {
    name: QueryParamNameMatcher,
    value: QueryParamValueMatcher,
}

impl Matcher<(&str, &str)> for QueryParamMatcher {
    fn matches(&self, (name, value): &(&str, &str)) -> bool {
        self.name.matches(name) && self.value.matches(value)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct RouteMatcher {
    // Match one hostname, method or path
    hostnames: Vec<HostnameMatcher>,
    methods: Vec<MethodMatcher>,
    paths: Vec<PathMatcher>,
    // Match all headers and query parameters
    headers: Vec<HeaderMatcher>,
    query_params: Vec<QueryParamMatcher>,
}

impl RouteMatcher {
    pub fn new_builder() -> RouteMatcherBuilder {
        RouteMatcherBuilder::new()
    }
}

#[derive(Default)]
pub struct RouteMatcherBuilder {
    hostnames: Vec<HostnameMatcher>,
    headers: Vec<HeaderMatcher>,
    methods: Vec<MethodMatcher>,
    paths: Vec<PathMatcher>,
    query_params: Vec<QueryParamMatcher>,
}

impl RouteMatcherBuilder {
    fn new() -> Self {
        RouteMatcherBuilder::default()
    }

    pub fn build(self) -> RouteMatcher {
        RouteMatcher {
            hostnames: self.hostnames,
            headers: self.headers,
            methods: self.methods,
            paths: self.paths,
            query_params: self.query_params,
        }
    }

    pub fn with_hostname(&mut self, hostname: String) -> &mut Self {
        self.hostnames
            .push(HostnameMatcher::Exact(CaseInsensitiveString::new(hostname)));
        self
    }

    pub fn with_hostname_suffix(&mut self, hostname: String) -> &mut Self {
        self.hostnames
            .push(HostnameMatcher::Suffix(CaseInsensitiveString::new(
                hostname,
            )));
        self
    }

    pub fn with_header(&mut self, name: HeaderName, value: HeaderValue) -> &mut Self {
        let header_matcher = HeaderMatcher {
            name: HeaderNameMatcher::new(name),
            value: HeaderValueMatcher::Exact(value),
        };
        self.headers.push(header_matcher);
        self
    }

    pub fn with_header_matching(&mut self, name: HeaderName, pattern: String) -> &mut Self {
        let header_matcher = HeaderMatcher {
            name: HeaderNameMatcher::new(name),
            value: HeaderValueMatcher::RegularExpression(pattern),
        };
        self.headers.push(header_matcher);
        self
    }

    pub fn with_method(&mut self, method: Method) -> &mut Self {
        self.methods.push(MethodMatcher(method));
        self
    }

    pub fn with_path(&mut self, path: String) -> &mut Self {
        self.paths.push(PathMatcher::Exact(path));
        self
    }

    pub fn with_path_prefix(&mut self, prefix: String) -> &mut Self {
        self.paths.push(PathMatcher::Prefix(prefix));
        self
    }

    pub fn with_path_matching(&mut self, pattern: String) -> &mut Self {
        self.paths.push(PathMatcher::RegularExpression(pattern));
        self
    }

    pub fn with_query_param(&mut self, name: String, value: String) -> &mut Self {
        self.query_params.push(QueryParamMatcher {
            name: QueryParamNameMatcher(name),
            value: QueryParamValueMatcher::Exact(value),
        });
        self
    }

    pub fn with_query_param_matching(&mut self, name: String, pattern: String) -> &mut Self {
        self.query_params.push(QueryParamMatcher {
            name: QueryParamNameMatcher(name),
            value: QueryParamValueMatcher::RegularExpression(pattern),
        });
        self
    }
}

impl Matcher<Parts> for RouteMatcher {
    fn matches(&self, part: &Parts) -> bool {
        if !self.methods.is_empty() &&  self.methods.iter().all(|m| !m.matches(&part.method)) {
            return false;
        }

        let path = part.uri.path();
        if !self.paths.is_empty() &&  self.paths.iter().all(|p| !p.matches(&path)) {
            return false;
        }

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
