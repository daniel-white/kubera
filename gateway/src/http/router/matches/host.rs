use super::CaseInsensitiveString;
use http::HeaderMap;
use tracing::{debug, instrument};

#[derive(Debug, PartialEq, Clone)]
pub enum HostValueMatch {
    Exact(CaseInsensitiveString),
    Suffix(CaseInsensitiveString),
}

impl HostValueMatch {
    #[instrument(
        skip(self, host),
        level = "debug",
        name = "HostValueMatch::matches"
        fields(match = ?self)
    )]
    fn matches(&self, host: &str) -> bool {
        match self {
            Self::Exact(expected) => expected == &CaseInsensitiveString::from(host),
            Self::Suffix(expected_suffix) => {
                CaseInsensitiveString::from(host).ends_with(expected_suffix.as_str())
            }
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
            .and_then(|v| v.to_str().ok())
        {
            Some(host) => self.host_value_matches.iter().any(|m| m.matches(host)),
            None => false,
        };

        if is_match {
            debug!("Host matched");
        }

        is_match
    }
}
