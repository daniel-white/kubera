use super::CaseInsensitiveString;
use super::Matcher;
use crate::http::router::matchers::score::Score;
use http::{HeaderMap, HeaderValue};
use tracing::{debug, instrument};

#[derive(Debug, PartialEq, Clone)]
pub enum HostHeaderValueMatcher {
    Exact(CaseInsensitiveString),
    Suffix(CaseInsensitiveString),
}

impl HostHeaderValueMatcher {
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
pub struct HostHeaderMatcher {
    pub matchers: Vec<HostHeaderValueMatcher>,
}

impl Matcher<HeaderMap> for HostHeaderMatcher {
    #[instrument(
        skip(self, score, headers),
        level = "debug",
        name = "HostHeaderMatcher::matches"
    )]
    fn matches(&self, score: &Score, headers: &HeaderMap) -> bool {
        let is_match = match headers.get(http_constant::HOST) {
            Some(host_header_value) => self
                .matchers
                .iter()
                .any(|matcher| matcher.matches(host_header_value)),
            None => false, // If there's no Host header, it doesn't match
        };

        if is_match {
            debug!("Host header matched");
            score.score_host_header(self);
        }

        is_match
    }
}
