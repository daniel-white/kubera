use crate::http::router::matches::{
    HostHeaderMatch, HostHeaderMatchBuilder, HttpRouteRuleMatchesBuilder, HttpRouteRuleMatchesScore,
};
use crate::http::router::{HttpBackend, HttpBackendBuilder, HttpRouteRuleMatches};
use getset::Getters;
use http::request::Parts;
use kubera_core::net::Hostname;
use tracing::instrument;

pub enum HttpRouteMatchResult<'a> {
    Matched(Vec<(&'a HttpRoute, &'a HttpRouteRule, HttpRouteRuleMatchesScore)>),
    NotMatched,
}

#[derive(Debug, Getters, Clone, PartialEq)]
pub struct HttpRoute {
    #[getset(get = "pub")]
    host_header_match: HostHeaderMatch,

    #[getset(get = "pub")]
    rules: Vec<HttpRouteRule>,
}

impl HttpRoute {
    pub fn new_builder() -> HttpRouteBuilder {
        HttpRouteBuilder::default()
    }

    #[instrument(skip(self, parts), level = "debug", name = "HttpRoute::matches")]
    pub fn matches(&self, parts: &Parts) -> HttpRouteMatchResult {
        if !self.host_header_match.matches(&parts.headers) {
            return HttpRouteMatchResult::NotMatched;
        }

        let matched_rules: Vec<_> = self
            .rules
            .iter()
            // .filter_map(|rule| match rule.matches().matches(parts) {
            //     HttpRouteRuleMatchesResult::Matched(score) => Some((self, rule, score)),
            //     _ => None,
            // })
            .collect();

        if matched_rules.is_empty() {
            return HttpRouteMatchResult::NotMatched;
        }

        HttpRouteMatchResult::NotMatched

        // HttpRouteMatchResult::Matched(matched_rules)
    }
}

#[derive(Default)]
pub struct HttpRouteBuilder {
    host_header_match_builder: HostHeaderMatchBuilder,
    rule_builders: Vec<HttpRouteRuleBuilder>,
}

impl HttpRouteBuilder {
    pub fn build(self) -> HttpRoute {
        HttpRoute {
            host_header_match: self.host_header_match_builder.build(),
            rules: self.rule_builders.into_iter().map(|b| b.build()).collect(),
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
        let mut builder = HttpRouteRuleBuilder::new(unique_id);
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
}

#[derive(Debug)]
pub struct HttpRouteRuleBuilder {
    unique_id: HttpRouteRuleUniqueId,
    matches_builders: Vec<HttpRouteRuleMatchesBuilder>,
    backend_builders: Vec<HttpBackendBuilder>,
}

impl HttpRouteRuleBuilder {
    pub fn new(unique_id: HttpRouteRuleUniqueId) -> Self {
        Self {
            unique_id,
            matches_builders: Vec::new(),
            backend_builders: Vec::new(),
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
        let mut backend_builder = HttpBackend::new_builder();
        factory(&mut backend_builder);
        self.backend_builders.push(backend_builder);
        self
    }
}
