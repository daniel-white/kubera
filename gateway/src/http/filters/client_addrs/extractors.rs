use http::request::Parts;
use http::HeaderName;
use ipnet::IpNet;
use std::net::{IpAddr, SocketAddr};
use trusted_proxies::{Config, Trusted};
use typed_builder::TypedBuilder;
use vg_core::http::filters::client_addrs::HttpClientAddrFilterKey;

pub trait ClientAddrExtractor {
    fn key(&self) -> &HttpClientAddrFilterKey;

    fn extract(&self, client_addr: SocketAddr, req: &Parts) -> Option<IpAddr>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientAddrExtractorType {
    Noop(NoopClientAddrExtractor),
    TrustedHeader(TrustedHeaderClientAddrExtractor),
    TrustedProxies(TrustedProxiesClientAddrExtractor),
}

impl ClientAddrExtractorType {
    pub fn extractor(&self) -> &dyn ClientAddrExtractor {
        match self {
            Self::Noop(extractor) => extractor,
            Self::TrustedHeader(extractor) => extractor,
            Self::TrustedProxies(extractor) => extractor,
        }
    }
}

#[derive(Debug, Clone, PartialEq, TypedBuilder, Eq)]
pub struct NoopClientAddrExtractor {
    key: HttpClientAddrFilterKey,
}

impl ClientAddrExtractor for NoopClientAddrExtractor {
    fn key(&self) -> &HttpClientAddrFilterKey {
        &self.key
    }

    fn extract(&self, _client_addr: SocketAddr, _req: &Parts) -> Option<IpAddr> {
        // TODO log
        None
    }
}

#[derive(Debug, Clone, PartialEq, TypedBuilder, Eq)]
pub struct TrustedHeaderClientAddrExtractor {
    #[builder(setter(into))]
    key: HttpClientAddrFilterKey,

    #[builder(setter(into))]
    header: HeaderName,
}

impl ClientAddrExtractor for TrustedHeaderClientAddrExtractor {
    fn key(&self) -> &HttpClientAddrFilterKey {
        &self.key
    }

    fn extract(&self, _client_addr: SocketAddr, req: &Parts) -> Option<IpAddr> {
        req.headers
            .get(&self.header)
            .and_then(|value| value.to_str().ok())
            .and_then(|s| s.parse::<IpAddr>().ok())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedProxiesClientAddrExtractor {
    key: HttpClientAddrFilterKey,
    trusted_ips: Vec<IpNet>,
    is_forwarded_trusted: bool,
    is_x_forwarded_for_trusted: bool,
    is_x_forwarded_host_trusted: bool,
    is_x_forwarded_proto_trusted: bool,
    is_x_forwarded_by_trusted: bool,
}

impl TrustedProxiesClientAddrExtractor {
    pub(crate) fn builder<K: Into<HttpClientAddrFilterKey>>(
        key: K,
    ) -> TrustedProxiesClientAddrExtractorBuilder {
        TrustedProxiesClientAddrExtractorBuilder {
            key: key.into(),
            trusted_ips: Vec::new(),
            is_forwarded_trusted: false,
            is_x_forwarded_for_trusted: false,
            is_x_forwarded_host_trusted: false,
            is_x_forwarded_proto_trusted: false,
            is_x_forwarded_by_trusted: false,
        }
    }
}

impl ClientAddrExtractor for TrustedProxiesClientAddrExtractor {
    fn key(&self) -> &HttpClientAddrFilterKey {
        &self.key
    }

    fn extract(&self, client_addr: SocketAddr, req: &Parts) -> Option<IpAddr> {
        let mut config = Config::new();
        for ip in &self.trusted_ips {
            let _ = config.add_trusted_ip(&ip.to_string());
        }
        if self.is_forwarded_trusted {
            config.trust_forwarded();
        }
        if self.is_x_forwarded_for_trusted {
            config.trust_x_forwarded_for();
        }
        if self.is_x_forwarded_host_trusted {
            config.trust_x_forwarded_host();
        }
        if self.is_x_forwarded_proto_trusted {
            config.trust_x_forwarded_proto();
        }
        if self.is_x_forwarded_by_trusted {
            config.trust_x_forwarded_by();
        }

        let trusted_ip = Trusted::from(client_addr.ip(), req, &config).ip();
        Some(trusted_ip)
    }
}

pub struct TrustedProxiesClientAddrExtractorBuilder {
    key: HttpClientAddrFilterKey,
    trusted_ips: Vec<IpNet>,
    is_forwarded_trusted: bool,
    is_x_forwarded_for_trusted: bool,
    is_x_forwarded_host_trusted: bool,
    is_x_forwarded_proto_trusted: bool,
    is_x_forwarded_by_trusted: bool,
}

impl TrustedProxiesClientAddrExtractorBuilder {
    pub fn build(self) -> TrustedProxiesClientAddrExtractor {
        TrustedProxiesClientAddrExtractor {
            key: self.key,
            trusted_ips: self.trusted_ips,
            is_forwarded_trusted: self.is_forwarded_trusted,
            is_x_forwarded_for_trusted: self.is_x_forwarded_for_trusted,
            is_x_forwarded_host_trusted: self.is_x_forwarded_host_trusted,
            is_x_forwarded_proto_trusted: self.is_x_forwarded_proto_trusted,
            is_x_forwarded_by_trusted: self.is_x_forwarded_by_trusted,
        }
    }

    pub fn add_trusted_ip(&mut self, ip: IpAddr) -> &mut Self {
        self.trusted_ips.push(ip.into());
        self
    }

    pub fn add_trusted_ip_range(&mut self, range: IpNet) -> &mut Self {
        self.trusted_ips.push(range);
        self
    }

    pub fn trust_forwarded_header(&mut self) -> &mut Self {
        self.is_forwarded_trusted = true;
        self
    }

    pub fn trust_x_forwarded_for_header(&mut self) -> &mut Self {
        self.is_x_forwarded_for_trusted = true;
        self
    }

    pub fn trust_x_forwarded_proto_header(&mut self) -> &mut Self {
        self.is_x_forwarded_proto_trusted = true;
        self
    }

    pub fn trust_x_forwarded_host_header(&mut self) -> &mut Self {
        self.is_x_forwarded_host_trusted = true;
        self
    }

    pub fn trust_x_forwarded_by_header(&mut self) -> &mut Self {
        self.is_x_forwarded_by_trusted = true;
        self
    }
}
