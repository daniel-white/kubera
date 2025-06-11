use derive_builder::Builder;
use getset::Getters;
use std::collections::BTreeMap;

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

#[derive(Clone, PartialEq, Debug)]
pub enum ResourceState<K: Clone> {
    Active(K),
    Deleted(K),
}

impl<K: Clone> ResourceState<K> {
    pub fn is_active(&self) -> bool {
        matches!(self, ResourceState::Active(_))
    }

    pub fn is_deleted(&self) -> bool {
        matches!(self, ResourceState::Deleted(_))
    }
}

#[derive(Getters, Default, Clone, PartialEq, Debug)]
pub struct Resources<K: Clone> {
    #[getset(get = "pub")]
    resources: BTreeMap<Ref, ResourceState<K>>,
}

impl<K: Clone> Resources<K> {
    pub fn set(&mut self, resource_ref: Ref, resource_state: ResourceState<K>) {
        self.resources.insert(resource_ref, resource_state);
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
        Self {
            resources: BTreeMap::from_iter(
                self.resources
                    .iter()
                    .filter(|(r, s)| f(r, s))
                    .map(|(r, s)| (r.clone(), s.clone())),
            ),
        }
    }
}
