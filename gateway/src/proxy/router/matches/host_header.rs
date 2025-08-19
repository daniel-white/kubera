use http::{HeaderMap, HeaderValue};
use tracing::{debug, instrument};
use vg_core::net::Hostname;

#[derive(Debug, PartialEq, Clone)]
pub enum HostHeaderValueMatch {
    Exact(Hostname),
    Suffix(Hostname),
}

impl HostHeaderValueMatch {
    #[instrument(
        skip(self, host_header_value),
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
    pub fn builder() -> HostHeaderMatchBuilder {
        HostHeaderMatchBuilder::new()
    }

    #[instrument(skip(self, headers), name = "HostHeaderMatch::matches")]
    pub fn matches(&self, headers: &HeaderMap) -> bool {
        if self.host_header_value_matches.is_empty() {
            debug!("No host header matches configured");
            return true;
        }

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

pub struct HostHeaderMatchBuilder {
    host_header_value_matches: Vec<HostHeaderValueMatch>,
}

impl HostHeaderMatchBuilder {
    fn new() -> Self {
        Self {
            host_header_value_matches: Vec::new(),
        }
    }

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
