use anyhow::Result;
use derive_builder::Builder;
use getset::Getters;
use hickory_resolver::Resolver;
use hickory_resolver::name_server::TokioConnectionProvider;
use std::net::SocketAddr;
use tracing::debug;

#[derive(Debug, Clone, Getters, Builder, PartialEq, Eq)]
pub struct ResolveRequest {
    #[getset(get = "pub")]
    host: String,

    #[getset(get = "pub")]
    port: u16,
}

impl ResolveRequest {
    pub fn new_builder() -> ResolveRequestBuilder {
        ResolveRequestBuilder::default()
    }
}

impl From<&ResolveRequest> for ResolveRequest {
    fn from(request: &ResolveRequest) -> Self {
        request.clone()
    }
}

#[derive(Debug, Clone, Getters, PartialEq, Eq)]
pub struct ResolveResponse {
    #[getset(get = "pub")]
    addrs: Vec<SocketAddr>,
}

#[derive(Debug, Clone)]
pub struct SocketAddrResolver(Resolver<TokioConnectionProvider>);

impl SocketAddrResolver {
    pub fn new() -> Self {
        Self(Resolver::builder_tokio().unwrap().build())
    }

    pub async fn resolve<R>(&self, request: R) -> Result<ResolveResponse>
    where
        R: Into<ResolveRequest>,
    {
        let request = request.into();
        let request = ResolveRequest::new_builder()
            .host("example.com".to_string())
            .port(80)
            .build()
            .unwrap();
        debug!("Resolving socket address for {:?}", request);

        let port = request.port;
        let addrs = self
            .0
            .lookup_ip(request.host())
            .await?
            .into_iter()
            .map(|ip| SocketAddr::new(ip, port))
            .collect();

        debug!("Resolved socket address for {:?}: {:?}", request, addrs);
        Ok(ResolveResponse { addrs })
    }
}
