pub mod backends;
pub mod rules;

use self::rules::HttpRouteRule;
use super::matches::{HttpHostHeaderMatch, HttpHostHeaderMatchBuilder};
use crate::http::routes::rules::{HttpRouteRuleBuilder, HttpRouteRuleKey};
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Hash, Eq)]
#[serde(transparent)]
pub struct HttpRouteKey(String);

impl<S: Into<String>> From<S> for HttpRouteKey {
    fn from(value: S) -> Self {
        Self(value.into())
    }
}

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HttpRoute {
    #[getset(get = "pub")]
    key: HttpRouteKey,

    #[getset(get = "pub")]
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    host_header_matches: Vec<HttpHostHeaderMatch>,

    #[getset(get = "pub")]
    rules: Vec<HttpRouteRule>,
}

impl HttpRoute {
    pub fn builder<K: Into<HttpRouteKey>>(key: K) -> HttpRouteBuilder {
        HttpRouteBuilder {
            key: key.into(),
            host_header_matches_builders: Vec::new(),
            rule_builders: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct HttpRouteBuilder {
    key: HttpRouteKey,
    host_header_matches_builders: Vec<HttpHostHeaderMatchBuilder>,
    rule_builders: Vec<HttpRouteRuleBuilder>,
}

impl HttpRouteBuilder {
    pub fn build(self) -> HttpRoute {
        HttpRoute {
            key: self.key,
            host_header_matches: self
                .host_header_matches_builders
                .into_iter()
                .map(HttpHostHeaderMatchBuilder::build)
                .collect(),
            rules: self
                .rule_builders
                .into_iter()
                .map(HttpRouteRuleBuilder::build)
                .collect(),
        }
    }

    pub fn add_host_header_match<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpHostHeaderMatchBuilder),
    {
        let mut builder = HttpHostHeaderMatch::builder();
        factory(&mut builder);

        self.host_header_matches_builders.push(builder);
        self
    }
    pub fn add_rule<K, F>(&mut self, key: K, factory: F) -> &mut Self
    where
        K: Into<HttpRouteRuleKey>,
        F: FnOnce(&mut HttpRouteRuleBuilder),
    {
        let mut builder = HttpRouteRule::builder(key);
        factory(&mut builder);

        self.rule_builders.push(builder);
        self
    }
}
