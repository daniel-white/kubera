use ipnet::IpNet;
use std::net::IpAddr;
use tracing::instrument;

#[derive(Debug, PartialEq)]
enum AccessControlFilterClientMatcher {
    Ip(IpAddr),
    IpRange(IpNet),
}

impl AccessControlFilterClientMatcher {
    pub fn matches(&self, client_addr: &IpAddr) -> bool {
        match self {
            AccessControlFilterClientMatcher::Ip(ip) => ip == client_addr,
            AccessControlFilterClientMatcher::IpRange(range) => range.contains(client_addr),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum AccessControlEvaluationResult {
    Allowed,
    Denied,
}

#[derive(Debug, PartialEq)]
pub struct AccessControlFilterHandler {
    allow_matchers: Vec<AccessControlFilterClientMatcher>,
    deny_matchers: Vec<AccessControlFilterClientMatcher>,
}

impl AccessControlFilterHandler {
    pub fn builder() -> AccessControlFilterHandlerBuilder {
        AccessControlFilterHandlerBuilder::new()
    }

    #[instrument(name = "AccessControlFilterHandler::evaluate", skip(self, client_addr))]
    pub fn evaluate(&self, client_addr: Option<IpAddr>) -> AccessControlEvaluationResult {
        if self.allow_matchers.is_empty() && self.deny_matchers.is_empty() {
            return AccessControlEvaluationResult::Allowed; // Default to allowed if no rules are defined
        }

        let Some(client_addr) = client_addr else {
            return AccessControlEvaluationResult::Denied; // If no client IP is provided, deny access
        };

        let is_allowed = self
            .allow_matchers
            .iter()
            .any(|matcher| matcher.matches(&client_addr));
        let is_denied = self
            .deny_matchers
            .iter()
            .any(|matcher| matcher.matches(&client_addr));

        if is_denied {
            AccessControlEvaluationResult::Denied // Any deny matcher takes precedence
        } else if is_allowed {
            AccessControlEvaluationResult::Allowed // If there's an allow matcher and no deny matches, allow access
        } else {
            AccessControlEvaluationResult::Denied // If no matchers apply, default to denied
        }
    }
}

pub struct AccessControlFilterHandlerBuilder {
    allow_matchers: Vec<AccessControlFilterClientMatcher>,
    deny_matchers: Vec<AccessControlFilterClientMatcher>,
}

impl AccessControlFilterHandlerBuilder {
    fn new() -> Self {
        Self {
            allow_matchers: Vec::new(),
            deny_matchers: Vec::new(),
        }
    }

    pub fn allow_ip(&mut self, ip: IpAddr) -> &Self {
        self.allow_matchers
            .push(AccessControlFilterClientMatcher::Ip(ip));
        self
    }

    pub fn allow_ip_range(&mut self, range: IpNet) -> &Self {
        self.allow_matchers
            .push(AccessControlFilterClientMatcher::IpRange(range));
        self
    }

    pub fn deny_ip(&mut self, ip: IpAddr) -> &Self {
        self.deny_matchers
            .push(AccessControlFilterClientMatcher::Ip(ip));
        self
    }

    pub fn deny_ip_range(&mut self, range: IpNet) -> &Self {
        self.deny_matchers
            .push(AccessControlFilterClientMatcher::IpRange(range));
        self
    }

    pub fn build(self) -> AccessControlFilterHandler {
        AccessControlFilterHandler {
            allow_matchers: self.allow_matchers,
            deny_matchers: self.deny_matchers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn ip(s: &str) -> IpAddr {
        IpAddr::from_str(s).unwrap()
    }
    fn ipnet(s: &str) -> IpNet {
        IpNet::from_str(s).unwrap()
    }

    #[test]
    fn test_no_matchers_ip_none() {
        let handler = AccessControlFilterHandler::builder().build();
        assert_eq!(
            handler.evaluate(None),
            AccessControlEvaluationResult::Allowed
        );
    }
    #[test]
    fn test_no_matchers_ip_some() {
        let handler = AccessControlFilterHandler::builder().build();
        assert_eq!(
            handler.evaluate(Some(ip("1.2.3.4"))),
            AccessControlEvaluationResult::Allowed
        );
    }
    #[test]
    fn test_allow_only_ip_none() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip(ip("1.2.3.4"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(None),
            AccessControlEvaluationResult::Denied
        );
    }
    #[test]
    fn test_allow_only_ip_some_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip(ip("1.2.3.4"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(Some(ip("1.2.3.4"))),
            AccessControlEvaluationResult::Allowed
        );
    }
    #[test]
    fn test_allow_only_ip_some_no_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip(ip("1.2.3.4"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(Some(ip("5.6.7.8"))),
            AccessControlEvaluationResult::Denied
        );
    }
    #[test]
    fn test_deny_only_ip_none() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.deny_ip(ip("1.2.3.4"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(None),
            AccessControlEvaluationResult::Denied
        );
    }
    #[test]
    fn test_deny_only_ip_some_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.deny_ip(ip("1.2.3.4"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(Some(ip("1.2.3.4"))),
            AccessControlEvaluationResult::Denied
        );
    }
    #[test]
    fn test_deny_only_ip_some_no_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.deny_ip(ip("1.2.3.4"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(Some(ip("5.6.7.8"))),
            AccessControlEvaluationResult::Denied
        );
    }
    #[test]
    fn test_allow_and_deny_ip_none() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip(ip("1.2.3.4"));
        handler.deny_ip(ip("5.6.7.8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(None),
            AccessControlEvaluationResult::Denied
        );
    }
    #[test]
    fn test_allow_and_deny_ip_some_deny_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip(ip("1.2.3.4"));
        handler.deny_ip(ip("1.2.3.4"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(Some(ip("1.2.3.4"))),
            AccessControlEvaluationResult::Denied
        );
    }
    #[test]
    fn test_allow_and_deny_ip_some_allow_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip(ip("1.2.3.4"));
        handler.deny_ip(ip("5.6.7.8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(Some(ip("1.2.3.4"))),
            AccessControlEvaluationResult::Allowed
        );
    }
    #[test]
    fn test_allow_and_deny_ip_some_no_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip(ip("1.2.3.4"));
        handler.deny_ip(ip("5.6.7.8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(Some(ip("9.9.9.9"))),
            AccessControlEvaluationResult::Denied
        );
    }
    #[test]
    fn test_allow_ip_range_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip_range(ipnet("10.0.0.0/8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(Some(ip("10.1.2.3"))),
            AccessControlEvaluationResult::Allowed
        );
    }

    #[test]
    fn test_allow_ip_range_no_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip_range(ipnet("10.0.0.0/8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(Some(ip("192.168.1.1"))),
            AccessControlEvaluationResult::Denied
        );
    }

    #[test]
    fn test_deny_ip_range_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.deny_ip_range(ipnet("10.0.0.0/8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(Some(ip("10.1.2.3"))),
            AccessControlEvaluationResult::Denied
        );
    }

    #[test]
    fn test_deny_ip_range_no_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.deny_ip_range(ipnet("10.0.0.0/8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(Some(ip("192.168.1.1"))),
            AccessControlEvaluationResult::Denied
        );
    }

    #[test]
    fn test_allow_and_deny_ip_range_overlap_deny_precedence() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip_range(ipnet("10.0.0.0/8"));
        handler.deny_ip_range(ipnet("10.1.0.0/16"));
        let handler = handler.build();
        // 10.1.2.3 is in both ranges, should be Denied
        assert_eq!(
            handler.evaluate(Some(ip("10.1.2.3"))),
            AccessControlEvaluationResult::Denied
        );
        // 10.2.2.2 is only in allow range, should be Allowed
        assert_eq!(
            handler.evaluate(Some(ip("10.2.2.2"))),
            AccessControlEvaluationResult::Allowed
        );
    }
}
