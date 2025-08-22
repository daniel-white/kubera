use http::HeaderName;
use ipnet::IpNet;
use pingora::prelude::Session;
use std::net::IpAddr;
use std::ops::Deref;
use std::sync::{Arc, LazyLock};
use trusted_proxies::{Config, Trusted};
use typed_builder::TypedBuilder;

pub trait ClientAddrExtractor {
    fn extract(&self, session: &Session) -> Option<IpAddr>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClientAddrExtractorType {
    Noop,
    TrustedHeader(Arc<TrustedHeaderClientAddrExtractor>),
    TrustedProxies(Arc<TrustedProxiesClientAddrExtractor>),
}

impl ClientAddrExtractorType {
    pub fn extractor(&self) -> &dyn ClientAddrExtractor {
        match self {
            Self::Noop => NOOP_CLIENT_ADDR_EXTRACTOR.deref(),
            Self::TrustedHeader(extractor) => extractor.as_ref(),
            Self::TrustedProxies(extractor) => extractor.as_ref(),
        }
    }
}


static NOOP_CLIENT_ADDR_EXTRACTOR: LazyLock<NoopClientAddrExtractor> = LazyLock::new(|| NoopClientAddrExtractor);

struct NoopClientAddrExtractor;

impl ClientAddrExtractor for NoopClientAddrExtractor {
    fn extract(&self, _session: &Session) -> Option<IpAddr> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, TypedBuilder)]
pub struct TrustedHeaderClientAddrExtractor {
    #[builder(setter(into))]
    header: HeaderName,
}

impl ClientAddrExtractor for TrustedHeaderClientAddrExtractor {
    fn extract(&self, session: &Session) -> Option<IpAddr> {
        session
            .get_header(&self.header)
            .and_then(|value| value.to_str().ok())
            .and_then(|s| s.parse::<IpAddr>().ok())
    }
}

#[derive(Debug, Clone)]
pub struct TrustedProxiesClientAddrExtractor {
    config: Config,
}

impl TrustedProxiesClientAddrExtractor {
    pub(crate) fn builder() -> TrustedProxiesClientAddrExtractorBuilder {
        TrustedProxiesClientAddrExtractorBuilder::new()
    }
}

impl ClientAddrExtractor for TrustedProxiesClientAddrExtractor {
    fn extract(&self, session: &Session) -> Option<IpAddr> {
        let client_addr = session.client_addr()?.as_inet()?;
        let trusted_ip = Trusted::from(
            client_addr.ip(),
            &session.req_header().as_owned_parts(),
            &self.config,
        )
        .ip();
        Some(trusted_ip)
    }
}

impl PartialEq for TrustedProxiesClientAddrExtractor {
    fn eq(&self, _other: &Self) -> bool {
        false // This extractor is not meant to be compared for equality, as it is stateful based on configuration.
    }
}

pub struct TrustedProxiesClientAddrExtractorBuilder {
    config: Config,
}

impl TrustedProxiesClientAddrExtractorBuilder {
    fn new() -> Self {
        Self {
            config: Config::new(),
        }
    }

    pub fn build(self) -> TrustedProxiesClientAddrExtractor {
        TrustedProxiesClientAddrExtractor {
            config: self.config,
        }
    }

    pub fn add_trusted_ip(&mut self, ip: IpAddr) -> &mut Self {
        self.config
            .add_trusted_ip(&ip.to_string())
            .expect("Failed to add trusted IP");
        self
    }

    pub fn add_trusted_ip_range(&mut self, range: IpNet) -> &mut Self {
        self.config
            .add_trusted_ip(&range.to_string())
            .expect("Failed to add trusted IP range");
        self
    }

    pub fn trust_forwarded_header(&mut self) {
        self.config.trust_forwarded();
    }

    pub fn trust_x_forwarded_for_header(&mut self) {
        self.config.trust_x_forwarded_for();
    }

    pub fn trust_x_forwarded_proto_header(&mut self) {
        self.config.trust_x_forwarded_proto();
    }

    pub fn trust_x_forwarded_host_header(&mut self) {
        self.config.trust_x_forwarded_host();
    }

    pub fn trust_x_forwarded_by_header(&mut self) {
        self.config.trust_x_forwarded_by();
    }
}
