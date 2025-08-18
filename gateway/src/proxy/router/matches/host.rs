use http::{HeaderMap, header::HOST};
use tracing::{debug, instrument};
use vg_core::net::Hostname;

#[derive(Debug, PartialEq, Clone)]
#[allow(dead_code)]
pub enum HostValueMatch {
    Exact(Hostname),
    Suffix(Hostname),
}

impl HostValueMatch {
    #[instrument(
        skip(self, host),
        level = "debug",
        name = "HostValueMatch::matches"
        fields(match = ?self)
    )]
    #[allow(dead_code)]
    fn matches(&self, host: &Hostname) -> bool {
        match self {
            Self::Exact(expected) => expected == host,
            Self::Suffix(expected_suffix) => host.ends_with(expected_suffix),
        }
    }
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct HostMatch {
    pub host_value_matches: Vec<HostValueMatch>,
}

impl HostMatch {
    #[instrument(skip(self, headers), level = "debug", name = "HostMatch::matches")]
    pub fn matches(&self, headers: &HeaderMap) -> bool {
        // If no host matches are defined, accept all requests
        if self.host_value_matches.is_empty() {
            debug!("No host matches defined, accepting all requests");
            return true;
        }

        let is_match = match headers
            .get(HOST)
            .and_then(|v| v.to_str().ok().map(Hostname::from))
        {
            Some(hostname) => self.host_value_matches.iter().any(|m| m.matches(&hostname)),
            None => false,
        };

        if is_match {
            debug!("Host matched");
        }

        is_match
    }
}
