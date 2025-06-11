use crate::net::resolver::ResolveRequest;
use derive_builder::Builder;
use getset::Getters;

#[derive(Debug, Clone, Builder, Getters, Hash, PartialEq, Eq)]
pub struct KubernetesService {
    #[getset(get = "pub")]
    namespace: String,
    #[getset(get = "pub")]
    name: String,
    #[getset(get = "pub")]
    port: u16,
}

impl KubernetesService {
    pub fn new_builder() -> KubernetesServiceBuilder {
        KubernetesServiceBuilder::default()
    }
}

impl From<&KubernetesService> for ResolveRequest {
    fn from(target: &KubernetesService) -> Self {
        ResolveRequest::new_builder()
            .host(format!(
                "{}.{}.svc.cluster.local",
                target.name, target.namespace
            ))
            .port(target.port)
            .build()
            .expect("Failed to create ResolveRequest from KubernetesService")
    }
}
