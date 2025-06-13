use super::CaseInsensitiveString;
use super::Match;
use crate::http::router::matches::score::MatchingScore;
use http::{HeaderMap, HeaderValue};
use tracing::{debug, instrument};

#[derive(Debug, PartialEq, Clone)]
pub enum HostHeaderValueMatch {
    Exact(CaseInsensitiveString),
    Suffix(CaseInsensitiveString),
}

impl HostHeaderValueMatch {
    #[instrument(
        skip(self, host_header_value),
        level = "debug",
        name = "HostHeaderValueMatcher::matches"
        fields(matcher = ?self)
    )]
    fn matches(&self, host_header_value: &HeaderValue) -> bool {
        match host_header_value.to_str().map(CaseInsensitiveString::from) {
            Ok(host) => match self {
                Self::Exact(expected) => expected == &host,
                Self::Suffix(expected) => host.ends_with(expected.as_str()),
            },
            Err(_) => false, // If the header value is not a valid UTF-8 string, it doesn't match
        }
    }
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct HostHeaderMatch {
    pub(super) host_header_value_matches: Vec<HostHeaderValueMatch>,
}

impl Match<HeaderMap> for HostHeaderMatch {
    #[instrument(
        skip(self, score, headers),
        level = "debug",
        name = "HostHeaderMatch::matches"
    )]
    fn matches(&self, score: &MatchingScore, headers: &HeaderMap) -> bool {
        let is_match = match headers.get(http_constant::HOST) {
            Some(host_header_value) => self
                .host_header_value_matches
                .iter()
                .any(|m| m.matches(host_header_value)),
            None => false, // If there's no Host header, it doesn't match
        };

        if is_match {
            debug!("Host header matched");
            score.host_header(self);
        }

        is_match
    }
}
