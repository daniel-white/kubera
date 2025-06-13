use super::score::MatchingScore;
use super::Match;
use crate::util::get_regex;
use getset::Getters;
use std::borrow::Cow;
use tracing::{debug, instrument};

#[derive(Debug, PartialEq, Clone)]
pub struct QueryParamNameMatch(String);

impl QueryParamNameMatch {
    fn matches(&self, name: &str) -> bool {
        self.0 == *name
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum QueryParamValueMatch {
    Exact(String),
    RegularExpression(String),
}

impl QueryParamValueMatch {
    fn matches(&self, value: &str) -> bool {
        match self {
            QueryParamValueMatch::Exact(expected_value) => expected_value == value,
            QueryParamValueMatch::RegularExpression(regex) => {
                let regex = get_regex(regex);
                regex.is_match(value)
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct QueryParamMatch {
    name_match: QueryParamNameMatch,
    value_match: QueryParamValueMatch,
}

impl QueryParamMatch {
    pub fn new_exact(name: &str, value: &str) -> Self {
        Self {
            name_match: QueryParamNameMatch(name.to_string()),
            value_match: QueryParamValueMatch::Exact(value.to_string()),
        }
    }

    pub fn new_matching(name: &str, pattern: &str) -> Self {
        Self {
            name_match: QueryParamNameMatch(name.to_string()),
            value_match: QueryParamValueMatch::RegularExpression(pattern.to_string()),
        }
    }

    #[instrument(
        skip(self, name, value),
        level = "debug",
        name = "QueryParamMatch::matches"
        fields(match = ?self)
    )]
    fn matches(&self, (name, value): &(Cow<str>, Cow<str>)) -> bool {
        self.name_match.matches(name.as_ref()) && self.value_match.matches(value.as_ref())
    }
}

#[derive(Debug, Getters, PartialEq, Default, Clone)]
pub struct QueryParamsMatch {
    pub(super) query_param_matches: Vec<QueryParamMatch>,
}

impl Match<Vec<(Cow<'_, str>, Cow<'_, str>)>> for QueryParamsMatch {
    #[instrument(
        skip(self, score, query_params),
        level = "debug",
        name = "QueryParamsMatch::matches"
    )]
    fn matches(&self, score: &MatchingScore, query_params: &Vec<(Cow<str>, Cow<str>)>) -> bool {
        let is_match = self.query_param_matches.iter().all(|m| {
            query_params
                .iter()
                .any(|query_param| m.matches(query_param))
        });
        if is_match {
            debug!("Query parameters matched");
            score.query_params(self, self.query_param_matches.len());
        }
        is_match
    }
}
