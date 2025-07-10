use http::{HeaderName, HeaderValue};
use ipnet::IpNet;
use kubera_core::config::gateway::types::net::{ClientAddrsSource, ProxyHeaders};
use kubera_core::config::gateway::types::GatewayConfiguration;
use kubera_core::continue_on;
use kubera_core::sync::signal::{channel, Receiver};
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
    configuration: &Receiver<Option<GatewayConfiguration>>,
) -> Receiver<ClientAddrFilter> {
    let (tx, rx) = channel(ClientAddrFilter::default());

    let configuration = configuration.clone();
    join_set.spawn(async move {
        loop {
            let filter = if let Some(config) = configuration.current().as_ref() {
                warn!("Using client address filter configuration: {:?}", config);
                match config.client_addrs().as_ref() {
                    None => ClientAddrFilter::default(),
                    Some(client_addrs) => match client_addrs.source() {
                        ClientAddrsSource::None => ClientAddrFilter::default(),
                        ClientAddrsSource::Header => {
                            if let Some(header_name) = client_addrs
                                .header()
                                .as_ref()
                                .and_then(|h| HeaderName::from_str(h).ok())
                            {
                                ClientAddrFilter::new(ClientAddrExtractorType::TrustedHeader(
                                    Arc::new(TrustedHeaderClientAddrExtractor::new(header_name)),
                                ))
                            } else {
                                ClientAddrFilter::default()
                            }
                        }
                        ClientAddrsSource::Proxies => {
                            if let Some(proxies) = client_addrs.proxies().as_ref() {
                                let mut extractor_builder =
                                    TrustedProxiesClientAddrExtractor::new_builder();

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

                                ClientAddrFilter::new(ClientAddrExtractorType::TrustedProxies(
                                    Arc::new(extractor),
                                ))
                            } else {
                                ClientAddrFilter::default()
                            }
                        }
                    },
                }
            } else {
                ClientAddrFilter::default()
            };

            debug!("Using client address filter: {:?}", filter);

            tx.replace(filter);

            continue_on!(configuration.changed());
        }
    });

    rx
}

#[derive(Debug, Clone, PartialEq)]
enum ClientAddrExtractorType {
    Default(Arc<DefaultClientAddrExtractor>),
    TrustedHeader(Arc<TrustedHeaderClientAddrExtractor>),
    TrustedProxies(Arc<TrustedProxiesClientAddrExtractor>),
}

impl ClientAddrExtractorType {
    pub fn extractor(&self) -> Arc<dyn ClientAddrExtractor> {
        match self {
            Self::Default(extractor) => extractor.clone(),
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

impl Default for ClientAddrFilter {
    fn default() -> Self {
        Self::new(ClientAddrExtractorType::Default(Arc::new(
            DefaultClientAddrExtractor,
        )))
    }
}

trait ClientAddrExtractor {
    fn extract(&self, session: &Session) -> Option<IpAddr>;
}

#[derive(Debug, Default, Clone, PartialEq)]
struct DefaultClientAddrExtractor;

impl ClientAddrExtractor for DefaultClientAddrExtractor {
    fn extract(&self, _session: &Session) -> Option<IpAddr> {
        None
    }
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
    fn new_builder() -> TrustedProxiesClientAddrExtractorBuilder {
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
