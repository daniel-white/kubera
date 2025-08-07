use crate::proxy::router::matches::{
    HostHeaderMatch, HostHeaderMatchBuilder, HttpRouteRuleMatchesBuilder, HttpRouteRuleMatchesScore,
};
use crate::proxy::router::topology::TopologyLocation;
use crate::proxy::router::{HttpBackend, HttpBackendBuilder, HttpRouteRuleMatches};
use getset::Getters;
use http::request::Parts;
use kubera_core::config::gateway::types::http::filters::HTTPRouteFilter;
use kubera_core::net::Hostname;
use std::sync::Arc;
use tracing::{debug, instrument};

/// Enhanced route match result that includes matched prefix context for redirect filters.
///
/// This struct is returned by the routing system when matching HTTP requests to routes.
/// It includes additional context about prefix matches that can be used by redirect filters
/// implementing `replacePrefixMatch` functionality.
///
/// # Fields
///
/// * `Matched` - Contains the matched rule, score, and optional matched prefix
/// * `NotMatched` - Indicates no route was matched
///
/// # Examples
///
/// ```rust,ignore
/// // For a route with prefix "/api/v1" matching request "/api/v1/users"
/// // matched_prefix will be Some("/api/v1")
/// // This allows redirect filters to replace "/api/v1" with a new prefix like "/v2"
/// // resulting in "/v2/users"
/// ```
#[derive(Debug, Clone)]
pub enum HttpRouteMatchResult {
    Matched {
        rule: Arc<HttpRouteRule>,
        score: HttpRouteRuleMatchesScore,
        /// The prefix that was matched from the route configuration.
        /// This is only populated for prefix-based route matches and is used
        /// by redirect filters implementing `replacePrefixMatch` functionality.
        ///
        /// - For prefix matches: Contains the matched prefix (e.g., "/api/v1")
        /// - For exact matches: Always None (exact matches don't support prefix replacement)
        /// - For regex matches: Always None (regex matches don't have simple prefixes)
        matched_prefix: Option<String>,
    },
    NotMatched,
}

impl HttpRouteMatchResult {
    /// Creates a successful route match result.
    ///
    /// # Arguments
    ///
    /// * `rule` - The matched HTTP route rule
    /// * `score` - The matching score for route precedence
    /// * `matched_prefix` - The matched prefix (if any) for redirect filters
    pub fn matched(
        rule: Arc<HttpRouteRule>,
        score: HttpRouteRuleMatchesScore,
        matched_prefix: Option<String>,
    ) -> Self {
        Self::Matched {
            rule,
            score,
            matched_prefix,
        }
    }

    /// Creates a failed route match result.
    pub fn not_matched() -> Self {
        Self::NotMatched
    }

    /// Check if the result represents a match
    pub fn is_matched(&self) -> bool {
        matches!(self, Self::Matched { .. })
    }

    /// Get the rule if matched
    pub fn rule(&self) -> Option<&Arc<HttpRouteRule>> {
        match self {
            Self::Matched { rule, .. } => Some(rule),
            Self::NotMatched => None,
        }
    }

    /// Get the score if matched
    pub fn score(&self) -> Option<&HttpRouteRuleMatchesScore> {
        match self {
            Self::Matched { score, .. } => Some(score),
            Self::NotMatched => None,
        }
    }

    /// Get the matched prefix if available
    pub fn matched_prefix(&self) -> Option<&String> {
        match self {
            Self::Matched { matched_prefix, .. } => matched_prefix.as_ref(),
            Self::NotMatched => None,
        }
    }
}

/// An HTTP route that can match incoming requests and determine which backend to route to.
///
/// Routes are composed of:
/// - Host header matching rules
/// - Multiple route rules with their own matching criteria
///
/// When a request is matched, the route returns the best matching rule along with
/// context information that can be used by filters (e.g., matched prefix for redirects).
#[derive(Debug, Getters, Clone, PartialEq)]
pub struct HttpRoute {
    #[getset(get = "pub")]
    host_header_match: HostHeaderMatch,

    #[getset(get = "pub")]
    rules: Vec<Arc<HttpRouteRule>>,
}

impl HttpRoute {
    /// Matches an HTTP request against this route's rules.
    ///
    /// This method performs the following steps:
    /// 1. Checks if the request's Host header matches the route's host requirements
    /// 2. Iterates through all route rules to find matches
    /// 3. Selects the best match based on routing precedence rules
    /// 4. Returns match result with context for filters (including matched prefix)
    ///
    /// # Arguments
    ///
    /// * `parts` - The HTTP request parts to match against
    ///
    /// # Returns
    ///
    /// An `HttpRouteMatchResult` containing:
    /// - Whether a match was found
    /// - The matched rule (if any)
    /// - The matching score for precedence
    /// - The matched prefix (for redirect filters using `replacePrefixMatch`)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let route = HttpRoute::new(/* ... */);
    /// let request_parts = /* HTTP request parts */;
    ///
    /// match route.matches(&request_parts) {
    ///     result if result.matched => {
    ///         // Route matched - can access result.rule and result.matched_prefix
    ///         if let Some(prefix) = result.matched_prefix {
    ///             // This was a prefix match - redirect filters can use this
    ///             println!("Matched prefix: {}", prefix);
    ///         }
    ///     }
    ///     _ => {
    ///         // No route matched
    ///     }
    /// }
    /// ```
    #[instrument(skip(self, parts), level = "debug", name = "HttpRoute::matches")]
    pub fn matches(&self, parts: &Parts) -> HttpRouteMatchResult {
        if !self.host_header_match.matches(&parts.headers) {
            return HttpRouteMatchResult::not_matched();
        }

        let best_match = self
            .rules
            .iter()
            .enumerate()
            .flat_map(|(i, rule)| {
                rule.matches()
                    .iter()
                    .enumerate()
                    .map(move |(j, m)| (format!("{i}:{j}"), rule, m))
            })
            .filter_map(|(path, rule, m)| {
                let match_result = m.matches(parts);
                if match_result.is_matched() {
                    debug!(
                        "Matched rule {:?} at path {} with score {:?} and prefix {:?}",
                        rule.unique_id,
                        path,
                        match_result.score(),
                        match_result.matched_prefix()
                    );
                    Some((path, rule, match_result))
                } else {
                    debug!("Rule {:?} at path {} did not match", rule.unique_id, path);
                    None
                }
            })
            .min_by(|(_, _, lhs), (_, _, rhs)| lhs.score().unwrap().cmp(rhs.score().unwrap()));

        match best_match {
            Some((path, rule, match_result)) => {
                debug!(
                    "Best match found for rule {:?} at path {} with prefix {:?}",
                    rule.unique_id,
                    path,
                    match_result.matched_prefix()
                );
                HttpRouteMatchResult::matched(
                    rule.clone(),
                    match_result.score().unwrap().clone(),
                    match_result.matched_prefix().cloned(),
                )
            }
            None => {
                debug!("No matching rule found for the request");
                HttpRouteMatchResult::not_matched()
            }
        }
    }
}

pub struct HttpRouteBuilder {
    current_location: Arc<TopologyLocation>,
    host_header_match_builder: HostHeaderMatchBuilder,
    rule_builders: Vec<HttpRouteRuleBuilder>,
}

impl HttpRouteBuilder {
    pub fn new(current_location: &Arc<TopologyLocation>) -> Self {
        HttpRouteBuilder {
            current_location: current_location.clone(),
            host_header_match_builder: HostHeaderMatch::builder(),
            rule_builders: Vec::new(),
        }
    }

    pub fn build(self) -> HttpRoute {
        HttpRoute {
            host_header_match: self.host_header_match_builder.build(),
            rules: self
                .rule_builders
                .into_iter()
                .map(|b| Arc::new(b.build()))
                .collect(),
        }
    }

    pub fn add_exact_host(&mut self, host: &Hostname) -> &mut Self {
        self.host_header_match_builder.with_exact_host(host);
        self
    }

    pub fn add_host_suffix(&mut self, host: &Hostname) -> &mut Self {
        self.host_header_match_builder.with_host_suffix(host);
        self
    }

    pub fn add_rule<F>(&mut self, unique_id: HttpRouteRuleUniqueId, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpRouteRuleBuilder),
    {
        let mut builder = HttpRouteRuleBuilder::new(unique_id, &self.current_location);
        factory(&mut builder);
        self.rule_builders.push(builder);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HttpRouteRuleUniqueId(String);

impl HttpRouteRuleUniqueId {
    pub fn new<S: Into<String>>(id: S) -> Self {
        Self(id.into())
    }
}

impl AsRef<str> for HttpRouteRuleUniqueId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<kubera_core::config::gateway::types::http::router::HttpRouteRuleUniqueId>
    for HttpRouteRuleUniqueId
{
    fn from(
        value: kubera_core::config::gateway::types::http::router::HttpRouteRuleUniqueId,
    ) -> Self {
        Self::new(value.get())
    }
}

impl From<&kubera_core::config::gateway::types::http::router::HttpRouteRuleUniqueId>
    for HttpRouteRuleUniqueId
{
    fn from(
        value: &kubera_core::config::gateway::types::http::router::HttpRouteRuleUniqueId,
    ) -> Self {
        Self::new(value.get())
    }
}

#[derive(Debug, Getters, Clone, PartialEq)]
pub struct HttpRouteRule {
    #[getset(get = "pub")]
    unique_id: HttpRouteRuleUniqueId,

    #[getset(get = "pub")]
    matches: Vec<HttpRouteRuleMatches>,

    #[getset(get = "pub")]
    backends: Vec<HttpBackend>,

    #[getset(get = "pub")]
    filters: Vec<HTTPRouteFilter>,
}

pub struct HttpRouteRuleBuilder {
    unique_id: HttpRouteRuleUniqueId,
    current_location: Arc<TopologyLocation>,
    matches_builders: Vec<HttpRouteRuleMatchesBuilder>,
    backend_builders: Vec<HttpBackendBuilder>,
    filters: Vec<HTTPRouteFilter>,
}

impl HttpRouteRuleBuilder {
    pub fn new(unique_id: HttpRouteRuleUniqueId, current_location: &Arc<TopologyLocation>) -> Self {
        Self {
            unique_id,
            current_location: current_location.clone(),
            matches_builders: Vec::new(),
            backend_builders: Vec::new(),
            filters: Vec::new(),
        }
    }

    pub fn build(self) -> HttpRouteRule {
        HttpRouteRule {
            unique_id: self.unique_id,
            matches: self
                .matches_builders
                .into_iter()
                .map(|b| b.build())
                .collect(),
            backends: self
                .backend_builders
                .into_iter()
                .map(|b| b.build())
                .collect(),
            filters: self.filters,
        }
    }

    pub fn add_matches<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpRouteRuleMatchesBuilder),
    {
        let mut matches_builder = HttpRouteRuleMatchesBuilder::default();
        factory(&mut matches_builder);
        self.matches_builders.push(matches_builder);
        self
    }

    pub fn add_backend<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpBackendBuilder),
    {
        let mut backend_builder = HttpBackendBuilder::new(&self.current_location);
        factory(&mut backend_builder);
        self.backend_builders.push(backend_builder);
        self
    }

    pub fn add_filter(&mut self, filter: HTTPRouteFilter) -> &mut Self {
        self.filters.push(filter);
        self
    }
}
