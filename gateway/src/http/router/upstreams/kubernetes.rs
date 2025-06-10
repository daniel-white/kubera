use crate::net::resolver::ResolveRequest;
use derive_builder::Builder;
use getset::Getters;

#[derive(Debug, Clone, Builder, Getters, Hash, PartialEq, Eq)]
pub struct KubernetesServiceTarget {
    #[getset(get = "pub")]
    namespace: String,
    #[getset(get = "pub")]
    name: String,
    #[getset(get = "pub")]
    port: u16,
}

impl KubernetesServiceTarget {
    pub fn new_builder() -> KubernetesServiceTargetBuilder {
        KubernetesServiceTargetBuilder::default()
    }
}

impl From<&KubernetesServiceTarget> for ResolveRequest {
    fn from(target: &KubernetesServiceTarget) -> Self {
        ResolveRequest::new_builder()
            .host(format!(
                "{}.{}.svc.cluster.local",
                target.name, target.namespace
            ))
            .port(target.port)
            .build()
            .expect("Failed to create ResolveRequest from KubernetesServiceTarget")
    }
}
