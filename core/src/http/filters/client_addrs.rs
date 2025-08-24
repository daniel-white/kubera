use getset::Getters;
use http::HeaderName;
use ipnet::IpNet;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use typed_builder::TypedBuilder;

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Hash, Eq)]
#[serde(transparent)]
pub struct HttpClientAddrFilterKey(String);

impl<S: AsRef<str>> From<S> for HttpClientAddrFilterKey {
    fn from(value: S) -> Self {
        Self(value.as_ref().to_string())
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TypedBuilder, Getters,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpClientAddrsFilterRef {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    key: HttpClientAddrFilterKey,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HttpClientAddrsSource {
    Header,
    Proxies,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Eq, Getters)]
#[serde(rename_all = "camelCase")]
pub struct HttpClientAddrsFilter {
    #[getset(get = "pub")]
    key: HttpClientAddrFilterKey,
    #[getset(get = "pub")]
    source: HttpClientAddrsSource,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "http_serde_ext::header_name::option"
    )]
    #[getset(get = "pub")]
    #[schemars(schema_with = "crate::schemars::http_header_name")]
    header: Option<HeaderName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub")]
    proxies: Option<HttpTrustedProxies>,
}

impl HttpClientAddrsFilter {
    pub fn builder() -> HttpClientAddrsFilterBuilder {
        HttpClientAddrsFilterBuilder {
            key: None,
            source: None,
            header: None,
            proxies: None,
        }
    }
}

#[derive(Debug)]
pub struct HttpClientAddrsFilterBuilder {
    key: Option<HttpClientAddrFilterKey>,
    source: Option<HttpClientAddrsSource>,
    header: Option<HeaderName>,
    proxies: Option<HttpTrustedProxiesBuilder>,
}

impl HttpClientAddrsFilterBuilder {
    pub fn key<K: Into<HttpClientAddrFilterKey>>(&mut self, key: K) -> &mut Self {
        self.key = Some(key.into());
        self
    }

    pub fn trust_header<H: Into<HeaderName>>(&mut self, header: H) -> &mut Self {
        self.source = Some(HttpClientAddrsSource::Header);
        self.header = Some(header.into());
        self.proxies = None;
        self
    }

    pub fn trust_proxies<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpTrustedProxiesBuilder),
    {
        self.source = Some(HttpClientAddrsSource::Proxies);
        let mut builder = HttpTrustedProxiesBuilder::new();
        factory(&mut builder);
        self.proxies = Some(builder);
        self
    }

    pub fn build(self) -> HttpClientAddrsFilter {
        let key = self.key.expect("key must be set");
        match self.source.expect("source must be set") {
            HttpClientAddrsSource::Header => HttpClientAddrsFilter {
                key,
                source: HttpClientAddrsSource::Header,
                header: self.header,
                proxies: None,
            },
            HttpClientAddrsSource::Proxies => HttpClientAddrsFilter {
                key,
                source: HttpClientAddrsSource::Proxies,
                header: None,
                proxies: self.proxies.map(HttpTrustedProxiesBuilder::build),
            },
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum HttpProxyHeaders {
    Forwarded,
    XForwardedFor,
    XForwardedHost,
    XForwardedProto,
    XForwardedBy,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Eq, Getters)]
pub struct HttpTrustedProxies {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[getset(get = "pub")]
    trusted_ips: Vec<IpAddr>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(schema_with = "crate::schemars::cidr_array")]
    #[getset(get = "pub")]
    trusted_ranges: Vec<IpNet>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[getset(get = "pub")]
    trusted_headers: Vec<HttpProxyHeaders>,
}

#[derive(Debug, Default)]
pub struct HttpTrustedProxiesBuilder {
    trusted_ips: Vec<IpAddr>,
    trusted_ranges: Vec<IpNet>,
    trusted_headers: Vec<HttpProxyHeaders>,
}

impl HttpTrustedProxiesBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn trust_local_ranges(&mut self) -> &mut Self {
        #[allow(clippy::unwrap_used)] // These are hardcoded and should not fail
        let mut ranges: Vec<_> = vec![
            // IPV4 Loopback
            "127.0.0.0/8".parse().unwrap(),
            // IPV4 Private Networks
            "10.0.0.0/8".parse().unwrap(),
            "172.16.0.0/12".parse().unwrap(),
            "192.168.0.0/16".parse().unwrap(),
            // IPV6 Loopback
            "::1/128".parse().unwrap(),
            // IPV6 Private network
            "fd00::/8".parse().unwrap(),
        ];
        self.trusted_ranges.append(&mut ranges);
        self
    }

    pub fn add_trusted_ip(&mut self, ip: IpAddr) -> &mut Self {
        self.trusted_ips.push(ip);
        self
    }

    pub fn add_trusted_range(&mut self, cidr: IpNet) -> &mut Self {
        self.trusted_ranges.push(cidr);
        self
    }

    pub fn add_trusted_header(&mut self, header: HttpProxyHeaders) -> &mut Self {
        self.trusted_headers.push(header);
        self
    }

    pub fn build(self) -> HttpTrustedProxies {
        HttpTrustedProxies {
            trusted_ips: self.trusted_ips,
            trusted_ranges: self.trusted_ranges,
            trusted_headers: self.trusted_headers,
        }
    }
}
