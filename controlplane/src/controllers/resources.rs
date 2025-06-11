use derive_builder::Builder;
use getset::Getters;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Builder, Getters, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[builder(setter(into))]
pub struct Ref {
    #[getset(get = "pub")]
    namespace: Option<String>,

    #[getset(get = "pub")]
    name: String,
}

impl Ref {
    pub fn new_builder() -> RefBuilder {
        RefBuilder::default()
    }
}

#[derive(PartialEq, Debug)]
pub enum ResourceState<K> {
    Active(K),
    Deleted(K),
}

impl<K> ResourceState<K> {
    pub fn is_active(&self) -> bool {
        matches!(self, ResourceState::Active(_))
    }

    pub fn is_deleted(&self) -> bool {
        matches!(self, ResourceState::Deleted(_))
    }
}

#[derive(Getters, Clone, Default, PartialEq, Debug)]
pub struct Resources<K> {
    #[getset(get = "pub")]
    resources: BTreeMap<Ref, Arc<ResourceState<K>>>,
}

impl<K> Resources<K> {
    pub fn set_active(&mut self, resource_ref: Ref, resource: K) {
        self.resources
            .insert(resource_ref, Arc::new(ResourceState::Active(resource)));
    }

    pub fn set_deleted(&mut self, resource_ref: Ref, resource: K) {
        self.resources
            .insert(resource_ref, Arc::new(ResourceState::Deleted(resource)));
    }

    pub fn is_active(&self, resource_ref: &Ref) -> bool {
        self.resources
            .get(resource_ref)
            .map_or(false, |state| state.is_active())
    }

    pub fn filter_into<F>(&self, f: F) -> Resources<K>
    where
        F: Fn(&Ref, &ResourceState<K>) -> bool,
    {
        let resources = BTreeMap::from_iter(
            self.resources
                .iter()
                .filter(|(r, s)| f(r, s))
                .map(|(r, s)| (r.clone(), s.clone())),
        );

        Self { resources }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Ref, &ResourceState<K>)> {
        self.resources.iter().map(|(r, s)| (r, s.as_ref()))
    }
}

#[derive(Debug, Builder, Getters, Clone, PartialEq)]
pub struct ServiceBackend {
    #[getset(get = "pub")]
    port: Option<i32>,
    #[getset(get = "pub")]
    weight: Option<i32>,
}

impl ServiceBackend {
    pub fn new_builder() -> ServiceBackendBuilder {
        ServiceBackendBuilder::default()
    }
}
