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
