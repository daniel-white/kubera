mod matches;

use crate::config::gateway::types::http::filters::HttpRouteFilter;
use crate::config::gateway::types::net::{Backend, BackendBuilder, BackendBuilderError};
use getset::Getters;
use itertools::{Either, Itertools};
pub use matches::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use thiserror::Error;

#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, JsonSchema,
)]
pub struct HttpRouteRuleUniqueId(#[getset(get = "pub")] String);

impl HttpRouteRuleUniqueId {
    pub fn new<S: AsRef<str>>(id: S) -> Self {
        Self(id.as_ref().to_string())
    }
}

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HttpRouteRule {
    #[getset(get = "pub")]
    unique_id: HttpRouteRuleUniqueId,

    #[getset(get = "pub")]
    #[validate(max_items = 16)]
    matches: Vec<HttpRouteRuleMatches>,

    #[getset(get = "pub")]
    #[validate(max_items = 16)]
    backends: Vec<Backend>,

    /// Filters array - matches Gateway API structure
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[validate(max_items = 16)]
    filters: Vec<HttpRouteFilter>,
}

#[derive(Debug, Error)]
pub enum HttpRouteRuleBuilderError {
    #[error("Invalid backend configuration at index {0}: {1}")]
    InvalidBackend(usize, BackendBuilderError),
}

#[derive(Debug)]
pub struct HttpRouteRuleBuilder {
    unique_id: HttpRouteRuleUniqueId,
    match_builders: Vec<HttpRouteRuleMatchesBuilder>,
    backend_builders: Vec<BackendBuilder>,
    filters: Vec<HttpRouteFilter>,
}

impl HttpRouteRuleBuilder {
    pub fn new<S: AsRef<str>>(unique_id: S) -> Self {
        Self {
            unique_id: HttpRouteRuleUniqueId::new(unique_id),
            match_builders: Vec::new(),
            backend_builders: Vec::new(),
            filters: Vec::new(),
        }
    }

    pub fn build(self) -> Result<HttpRouteRule, HttpRouteRuleBuilderError> {
        let (backends, errs): (Vec<_>, Vec<_>) = self
            .backend_builders
            .into_iter()
            .enumerate()
            .map(|(i, b)| (i, b.build()))
            .partition_map(|(i, r)| match r {
                Ok(backend) => Either::Left(backend),
                Err(err) => Either::Right(HttpRouteRuleBuilderError::InvalidBackend(i, err)),
            });

        if let Some(err) = errs.into_iter().next() {
            return Err(err);
        }

        Ok(HttpRouteRule {
            unique_id: self.unique_id,
            matches: self
                .match_builders
                .into_iter()
                .map(HttpRouteRuleMatchesBuilder::build)
                .collect(),
            backends,
            filters: self.filters,
        })
    }

    pub fn add_match<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpRouteRuleMatchesBuilder),
    {
        let mut match_builder = HttpRouteRuleMatchesBuilder::default();
        factory(&mut match_builder);
        self.match_builders.push(match_builder);
        self
    }

    pub fn add_backend<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut BackendBuilder),
    {
        let mut backend_builder = BackendBuilder::default();
        factory(&mut backend_builder);
        self.backend_builders.push(backend_builder);
        self
    }

    /// Add a filter to the filters array
    pub fn add_filter(&mut self, filter: HttpRouteFilter) -> &mut Self {
        self.filters.push(filter);
        self
    }
}

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HttpRoute {
    #[getset(get = "pub")]
    #[validate(max_items = 16)]
    #[serde(
        rename = "host_headers",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    host_header_matches: Vec<HostHeaderMatch>,

    #[getset(get = "pub")]
    #[validate(max_items = 16)]
    rules: Vec<HttpRouteRule>,
}

#[derive(Debug, Error)]
pub enum HttpRouteBuilderError {
    #[error("Invalid rule at index {0}: {1}")]
    InvalidRule(usize, HttpRouteRuleBuilderError),
}

#[derive(Debug, Default)]
pub struct HttpRouteBuilder {
    host_header_matches: Vec<HostHeaderMatch>,
    rule_builders: Vec<HttpRouteRuleBuilder>,
}

impl HttpRouteBuilder {
    pub fn build(self) -> Result<HttpRoute, HttpRouteBuilderError> {
        let (rules, errs): (Vec<_>, Vec<_>) = self
            .rule_builders
            .into_iter()
            .enumerate()
            .map(|(i, b)| (i, b.build()))
            .partition_map(|(i, r)| match r {
                Ok(rule) => Either::Left(rule),
                Err(err) => Either::Right(HttpRouteBuilderError::InvalidRule(i, err)),
            });

        if let Some(err) = errs.into_iter().next() {
            return Err(err);
        }

        Ok(HttpRoute {
            host_header_matches: self.host_header_matches,
            rules,
        })
    }

    pub fn add_exact_host_header<S: AsRef<str>>(&mut self, host: S) -> &mut Self {
        let host_header_match = HostHeaderMatch::exactly(host);
        self.host_header_matches.push(host_header_match);
        self
    }

    pub fn add_host_header_with_suffix<S: AsRef<str>>(&mut self, host: S) -> &mut Self {
        let host_header_match = HostHeaderMatch::with_suffix(host);
        self.host_header_matches.push(host_header_match);
        self
    }

    pub fn add_rule<S: AsRef<str>, F>(&mut self, unique_id: S, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpRouteRuleBuilder),
    {
        let mut rule_builder = HttpRouteRuleBuilder::new(unique_id);
        factory(&mut rule_builder);
        self.rule_builders.push(rule_builder);
        self
    }
}
