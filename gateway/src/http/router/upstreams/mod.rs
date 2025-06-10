mod kubernetes;

use crate::net::resolver::ResolveRequest;
use derive_builder::Builder;
use getset::Getters;
use tracing::debug;

#[derive(Debug, Clone, PartialEq)]
pub enum TransportSecurity {
    None,
    Tls,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpstreamTarget {
    KubernetesService(kubernetes::KubernetesServiceTarget),
}

#[derive(Debug, Builder, Getters, Clone, PartialEq)]
pub struct Upstream {
    #[getset(get = "pub")]
    target: UpstreamTarget,
    #[getset(get = "pub")]
    transport_security: TransportSecurity,
}

impl Upstream {
    pub fn new_builder() -> UpstreamBuilder {
        UpstreamBuilder::default()
    }
}

impl From<&Upstream> for ResolveRequest {
    fn from(upstream: &Upstream) -> Self {
        match &upstream.target {
            UpstreamTarget::KubernetesService(target) => target.into(),
        }
    }
}

impl UpstreamBuilder {
    pub fn kubernetes_service(&mut self, namespace: String, name: String, port: u16) -> &mut Self {
        debug!(
            "Creating KubernetesServiceTarget with namespace: {}, name: {}, port: {}",
            namespace, name, port
        );
        let target = kubernetes::KubernetesServiceTarget::new_builder()
            .namespace(namespace)
            .name(name)
            .port(port)
            .build()
            .expect("Failed to build KubernetesServiceTarget");

        self.target(UpstreamTarget::KubernetesService(target))
    }
}

#[derive(Debug, Default)]
pub struct UpstreamsBuilder {
    upstreams: Vec<Upstream>,
}

impl UpstreamsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_upstream(&mut self, upstream: Upstream) -> &mut Self {
        debug!("Added upstream: {:?}", upstream);
        self.upstreams.push(upstream);
        self
    }

    pub fn build(self) -> Vec<Upstream> {
        self.upstreams
    }
}
