use super::Matcher;
use super::score::Score;
use crate::util::get_regex;
use getset::Getters;
use std::borrow::Cow;

#[derive(Debug, PartialEq, Clone)]
pub struct QueryParamNameMatcher(String);

impl QueryParamNameMatcher {
    fn matches(&self, name: &str) -> bool {
        self.0 == *name
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum QueryParamValueMatcher {
    Exact(String),
    RegularExpression(String),
}

impl QueryParamValueMatcher {
    fn matches(&self, value: &str) -> bool {
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
pub struct QueryParamMatcher {
    name_matcher: QueryParamNameMatcher,
    value_matcher: QueryParamValueMatcher,
}

impl QueryParamMatcher {
    pub fn new(name: &str, value: &str) -> Self {
        Self {
            name_matcher: QueryParamNameMatcher(name.to_string()),
            value_matcher: QueryParamValueMatcher::Exact(value.to_string()),
        }
    }

    pub fn new_matching(name: &str, pattern: &str) -> Self {
        Self {
            name_matcher: QueryParamNameMatcher(name.to_string()),
            value_matcher: QueryParamValueMatcher::RegularExpression(pattern.to_string()),
        }
    }

    fn matches(&self, (name, value): &(Cow<str>, Cow<str>)) -> bool {
        self.name_matcher.matches(name.as_ref()) && self.value_matcher.matches(value.as_ref())
    }
}

#[derive(Debug, Getters, PartialEq, Default, Clone)]
pub struct QueryParamsMatcher {
    pub matchers: Vec<QueryParamMatcher>,
}

impl Matcher<Vec<(Cow<'_, str>, Cow<'_, str>)>> for QueryParamsMatcher {
    fn matches(&self, score: &Score, query_params: &Vec<(Cow<str>, Cow<str>)>) -> bool {
        let is_match = self
            .matchers
            .iter()
            .all(|matcher| query_params.iter().any(|param| matcher.matches(param)));
        if is_match {
            score.score_query_params(self);
        }
        is_match
    }
}
