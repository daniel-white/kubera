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
pub enum BackendTarget {
    KubernetesService(kubernetes::KubernetesService),
}

#[derive(Debug, Builder, Getters, Clone, PartialEq)]
pub struct Backend {
    #[getset(get = "pub")]
    target: BackendTarget,
    #[getset(get = "pub")]
    transport_security: TransportSecurity,
}

impl Backend {
    pub fn new_builder() -> BackendBuilder {
        BackendBuilder::default()
    }
}

impl From<&Backend> for ResolveRequest {
    fn from(backend: &Backend) -> Self {
        match &backend.target {
            BackendTarget::KubernetesService(target) => target.into(),
        }
    }
}

impl BackendBuilder {
    pub fn kubernetes_service(&mut self, namespace: String, name: String, port: u16) -> &mut Self {
        debug!(
            "Creating KubernetesServiceTarget with namespace: {}, name: {}, port: {}",
            namespace, name, port
        );
        let target = kubernetes::KubernetesService::new_builder()
            .namespace(namespace)
            .name(name)
            .port(port)
            .build()
            .expect("Failed to build KubernetesServiceTarget");

        self.target(BackendTarget::KubernetesService(target))
    }
}

#[derive(Debug, Default)]
pub struct BackendsBuilder {
    backends: Vec<Backend>,
}

impl BackendsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_backend(&mut self, backend: Backend) -> &mut Self {
        debug!("Added backends: {:?}", backend);
        self.backends.push(backend);
        self
    }

    pub fn build(self) -> Vec<Backend> {
        self.backends
    }
}
