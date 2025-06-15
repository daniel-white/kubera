use http::HeaderMap;
use kubera_core::net::Hostname;
use tracing::{debug, instrument};

#[derive(Debug, PartialEq, Clone)]
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
    fn matches(&self, headers: &HeaderMap) -> bool {
        let is_match = match headers
            .get(http_constant::HOST)
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
