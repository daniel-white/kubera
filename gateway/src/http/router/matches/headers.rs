use super::Match;
use super::score::HttpRouteRuleMatchesScore;
use crate::util::get_regex;
use getset::Getters;
use http::{HeaderMap, HeaderName, HeaderValue};
use tracing::{debug, instrument};

#[derive(Debug, PartialEq, Clone)]
pub struct HeaderNameMatch(HeaderName);

impl HeaderNameMatch {
    fn matches(&self, name: &HeaderName) -> bool {
        self.0 == *name
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum HeaderValueMatch {
    Exact(HeaderValue),
    RegularExpression(String),
}

impl HeaderValueMatch {
    fn matches(&self, value: &HeaderValue) -> bool {
        match self {
            HeaderValueMatch::Exact(expected_value) => expected_value == value,
            HeaderValueMatch::RegularExpression(regex) => match value.to_str() {
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
pub struct HeaderMatch {
    name_match: HeaderNameMatch,
    value_match: HeaderValueMatch,
}

impl HeaderMatch {
    pub fn new_exact(name: HeaderName, value: HeaderValue) -> Self {
        Self {
            name_match: HeaderNameMatch(name),
            value_match: HeaderValueMatch::Exact(value),
        }
    }

    pub fn new_matching(name: HeaderName, pattern: &str) -> Self {
        Self {
            name_match: HeaderNameMatch(name),
            value_match: HeaderValueMatch::RegularExpression(pattern.to_string()),
        }
    }

    #[instrument(
        skip(self, name, value),
        level = "debug",
        name = "HeaderMatch::matches"
        fields(match = ?self)
    )]
    fn matches(&self, (name, value): &(&HeaderName, &HeaderValue)) -> bool {
        self.name_match.matches(name) && self.value_match.matches(value)
    }
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct HeadersMatch {
    pub(super) header_matches: Vec<HeaderMatch>,
}

impl Match<HeaderMap> for HeadersMatch {
    #[instrument(
        skip(self, score, headers),
        level = "debug",
        name = "HeadersMatch::matches"
    )]
    fn matches(&self, score: &HttpRouteRuleMatchesScore, headers: &HeaderMap) -> bool {
        let is_match = self
            .header_matches
            .iter()
            .all(|m| headers.iter().any(|header| m.matches(&header)));
        if is_match {
            debug!("Headers matched");
            score.headers(self, self.header_matches.len());
        }
        is_match
    }
}
