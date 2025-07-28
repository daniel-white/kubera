use http::{HeaderName, HeaderValue};
use ipnet::IpNet;
use kubera_core::config::gateway::types::net::{ClientAddrsSource, ProxyHeaders};
use kubera_core::config::gateway::types::GatewayConfiguration;
use kubera_core::continue_on;
use kubera_core::sync::signal::{signal, Receiver};
use pingora::proxy::Session;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{debug, warn};
use trusted_proxies::{Config, Trusted};

const KUBERA_CLIENT_IP_HEADER: HeaderName = HeaderName::from_static("kubera-client-ip");

pub fn client_addr_filter(
    join_set: &mut JoinSet<()>,
    gateway_configuration_rx: &Receiver<GatewayConfiguration>,
) -> Receiver<ClientAddrFilter> {
    let (tx, rx) = signal();
    let gateway_configuration_rx = gateway_configuration_rx.clone();

    join_set.spawn(async move {
        loop {
            if let Some(gateway_configuration) = gateway_configuration_rx.get().await
                && let Some(client_addrs) = gateway_configuration.client_addrs()
            {
                let filter = match client_addrs.source() {
                    ClientAddrsSource::None => None,
                    ClientAddrsSource::Header => client_addrs
                        .header()
                        .as_ref()
                        .and_then(|h| HeaderName::from_str(h).ok())
                        .map(|header_name| {
                            ClientAddrFilter::new(ClientAddrExtractorType::TrustedHeader(Arc::new(
                                TrustedHeaderClientAddrExtractor::new(header_name),
                            )))
                        }),
                    ClientAddrsSource::Proxies => {
                        if let Some(proxies) = client_addrs.proxies().as_ref() {
                            let mut extractor_builder =
                                TrustedProxiesClientAddrExtractor::builder();

                            for ip in proxies.trusted_ips().iter().cloned() {
                                extractor_builder.add_trusted_ip(ip);
                            }

                            for range in proxies.trusted_ranges().iter().cloned() {
                                extractor_builder.add_trusted_ip_range(range);
                            }

                            for header in proxies.trusted_headers() {
                                match header {
                                    ProxyHeaders::Forwarded => {
                                        extractor_builder.trust_forwarded_header()
                                    }
                                    ProxyHeaders::XForwardedFor => {
                                        extractor_builder.trust_x_forwarded_for_header()
                                    }
                                    ProxyHeaders::XForwardedProto => {
                                        extractor_builder.trust_x_forwarded_proto_header()
                                    }
                                    ProxyHeaders::XForwardedHost => {
                                        extractor_builder.trust_x_forwarded_host_header()
                                    }
                                    ProxyHeaders::XForwardedBy => {
                                        extractor_builder.trust_x_forwarded_by_header()
                                    }
                                }
                            }

                            let extractor = extractor_builder.build();

                            Some(ClientAddrFilter::new(
                                ClientAddrExtractorType::TrustedProxies(Arc::new(extractor)),
                            ))
                        } else {
                            None
                        }
                    }
                };

                tx.replace(filter).await;
            }

            continue_on!(gateway_configuration_rx.changed());
        }
    });

    rx
}

#[derive(Debug, Clone, PartialEq)]
enum ClientAddrExtractorType {
    TrustedHeader(Arc<TrustedHeaderClientAddrExtractor>),
    TrustedProxies(Arc<TrustedProxiesClientAddrExtractor>),
}

impl ClientAddrExtractorType {
    pub fn extractor(&self) -> Arc<dyn ClientAddrExtractor> {
        match self {
            Self::TrustedHeader(extractor) => extractor.clone(),
            Self::TrustedProxies(extractor) => extractor.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClientAddrFilter {
    extractor: ClientAddrExtractorType,
}

impl ClientAddrFilter {
    fn new(extractor: ClientAddrExtractorType) -> Self {
        Self { extractor }
    }

    pub fn filter(&self, session: &mut Session) {
        let extractor = self.extractor.extractor();
        if let Some(client_addr) = extractor.extract(session) {
            let headers = session.req_header_mut();
            headers
                .insert_header(
                    KUBERA_CLIENT_IP_HEADER,
                    HeaderValue::from_str(&client_addr.to_string()).unwrap(),
                )
                .unwrap_or_else(|err| {
                    warn!(
                        "Failed to insert header {}: {}",
                        KUBERA_CLIENT_IP_HEADER, err
                    );
                });
        } else {
            let headers = session.req_header_mut();
            headers.remove_header(&KUBERA_CLIENT_IP_HEADER); // **MUST** remove the header from the client if the address is not available
        }
    }
}

trait ClientAddrExtractor {
    fn extract(&self, session: &Session) -> Option<IpAddr>;
}

#[derive(Debug, Clone, PartialEq)]
struct TrustedHeaderClientAddrExtractor {
    header: HeaderName,
}

impl TrustedHeaderClientAddrExtractor {
    pub fn new(header: HeaderName) -> Self {
        Self { header }
    }
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
struct TrustedProxiesClientAddrExtractor {
    config: Config,
}

impl TrustedProxiesClientAddrExtractor {
    fn builder() -> TrustedProxiesClientAddrExtractorBuilder {
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

struct TrustedProxiesClientAddrExtractorBuilder {
    config: Config,
}

impl TrustedProxiesClientAddrExtractorBuilder {
    fn new() -> Self {
        Self {
            config: Config::new(),
        }
    }

    fn build(self) -> TrustedProxiesClientAddrExtractor {
        TrustedProxiesClientAddrExtractor {
            config: self.config,
        }
    }

    fn add_trusted_ip(&mut self, ip: IpAddr) -> &mut Self {
        self.config
            .add_trusted_ip(&ip.to_string())
            .expect("Failed to add trusted IP");
        self
    }

    fn add_trusted_ip_range(&mut self, range: IpNet) -> &mut Self {
        self.config
            .add_trusted_ip(&range.to_string())
            .expect("Failed to add trusted IP range");
        self
    }

    fn trust_forwarded_header(&mut self) {
        self.config.trust_forwarded();
    }

    fn trust_x_forwarded_for_header(&mut self) {
        self.config.trust_x_forwarded_for();
    }

    fn trust_x_forwarded_proto_header(&mut self) {
        self.config.trust_x_forwarded_proto();
    }

    fn trust_x_forwarded_host_header(&mut self) {
        self.config.trust_x_forwarded_host();
    }

    fn trust_x_forwarded_by_header(&mut self) {
        self.config.trust_x_forwarded_by();
    }
}
