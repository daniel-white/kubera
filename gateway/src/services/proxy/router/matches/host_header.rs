use http::{HeaderMap, HeaderValue};
use tracing::{debug, instrument};
use kubera_core::net::Hostname;

#[derive(Debug, PartialEq, Clone)]
pub enum HostHeaderValueMatch {
    Exact(Hostname),
    Suffix(Hostname),
}

impl HostHeaderValueMatch {
    #[instrument(
        skip(self, host_header_value),
        level = "debug",
        name = "HostHeaderValueMatcher::matches"
        fields(matcher = ?self)
    )]
    fn matches(&self, host_header_value: &HeaderValue) -> bool {
        match host_header_value.to_str().map(Hostname::from) {
            Ok(host) => match self {
                Self::Exact(expected) => expected == &host,
                Self::Suffix(expected) => host.ends_with(expected),
            },
            Err(_) => false, // If the header value is not a valid UTF-8 string, it doesn't match
        }
    }
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct HostHeaderMatch {
    host_header_value_matches: Vec<HostHeaderValueMatch>,
}

impl HostHeaderMatch {
    pub fn new_builder() -> HostHeaderMatchBuilder {
        HostHeaderMatchBuilder::default()
    }

    #[instrument(
        skip(self, headers),
        level = "debug",
        name = "HostHeaderMatch::matches"
    )]
    pub fn matches(&self, headers: &HeaderMap) -> bool {
        let is_match = match headers.get(http_constant::HOST) {
            Some(host_header_value) => self
                .host_header_value_matches
                .iter()
                .any(|m| m.matches(host_header_value)),
            None => false, // If there's no Host header, it doesn't match
        };

        if is_match {
            debug!("Host header matched");
        }

        is_match
    }
}

#[derive(Default)]
pub struct HostHeaderMatchBuilder {
    host_header_value_matches: Vec<HostHeaderValueMatch>,
}

impl HostHeaderMatchBuilder {
    pub fn build(self) -> HostHeaderMatch {
        HostHeaderMatch {
            host_header_value_matches: self.host_header_value_matches,
        }
    }

    pub fn with_exact_host(&mut self, host: &Hostname) {
        let host_header_value_match = HostHeaderValueMatch::Exact(host.clone());
        self.host_header_value_matches.push(host_header_value_match);
    }

    pub fn with_host_suffix(&mut self, host: &Hostname) {
        let host_header_value_match = HostHeaderValueMatch::Suffix(host.clone());
        self.host_header_value_matches.push(host_header_value_match);
    }
}
