use super::score::Score;
use super::Matcher;
use crate::util::get_regex;
use getset::Getters;
use http::{HeaderMap, HeaderName, HeaderValue};
use tracing::{debug, instrument};

#[derive(Debug, PartialEq, Clone)]
pub struct HeaderNameMatcher(HeaderName);

impl HeaderNameMatcher {
    fn matches(&self, name: &HeaderName) -> bool {
        self.0 == *name
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum HeaderValueMatcher {
    Exact(HeaderValue),
    RegularExpression(String),
}

impl HeaderValueMatcher {
    fn matches(&self, value: &HeaderValue) -> bool {
        match self {
            HeaderValueMatcher::Exact(expected_value) => expected_value == value,
            HeaderValueMatcher::RegularExpression(regex) => match value.to_str() {
                Ok(value) => {
                    let regex = get_regex(regex);
                    regex.is_match(value)
                }
                Err(_) => false,
            },
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct HeaderMatcher {
    name_matcher: HeaderNameMatcher,
    value_matcher: HeaderValueMatcher,
}

impl HeaderMatcher {
    pub fn new(name: HeaderName, value: HeaderValue) -> Self {
        Self {
            name_matcher: HeaderNameMatcher(name),
            value_matcher: HeaderValueMatcher::Exact(value),
        }
    }

    pub fn new_matching(name: HeaderName, pattern: &str) -> Self {
        Self {
            name_matcher: HeaderNameMatcher(name),
            value_matcher: HeaderValueMatcher::RegularExpression(pattern.to_string()),
        }
    }

    #[instrument(
        skip(self, name, value),
        level = "debug",
        name = "HeaderMatcher::matches"
        fields(matcher = ?self)
    )]
    fn matches(&self, (name, value): &(&HeaderName, &HeaderValue)) -> bool {
        self.name_matcher.matches(name) && self.value_matcher.matches(value)
    }
}

#[derive(Debug, Getters, PartialEq, Default, Clone)]
pub struct HeadersMatcher {
    pub matchers: Vec<HeaderMatcher>,
}

impl Matcher<HeaderMap> for HeadersMatcher {
    #[instrument(
        skip(self, score, headers),
        level = "debug",
        name = "HeadersMatcher::matches"
    )]
    fn matches(&self, score: &Score, headers: &HeaderMap) -> bool {
        let is_match = self
            .matchers
            .iter()
            .all(|matcher| headers.iter().any(|header| matcher.matches(&header)));
        if is_match {
            score.score_headers(self);
            debug!("Headers matched");
        }
        is_match
    }
}
