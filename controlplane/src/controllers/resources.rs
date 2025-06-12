use derive_builder::Builder;
use getset::Getters;
use kube::runtime::reflector::Lookup;
use kube::{Resource, ResourceExt};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter, Write};
use std::sync::Arc;

#[derive(Builder, Getters, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[builder(setter(into))]
pub struct ObjectRef {
    #[getset(get = "pub")]
    kind: String,

    #[getset(get = "pub")]
    version: String,

    #[getset(get = "pub")]
    group: Option<String>,

    #[getset(get = "pub")]
    namespace: Option<String>,

    #[getset(get = "pub")]
    name: String,
}

impl ObjectRef {
    pub fn new_builder() -> ObjectRefBuilder {
        ObjectRefBuilder::default()
    }
}

impl Display for ObjectRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.kind())?;
        f.write_char('.')?;
        f.write_str(self.version())?;
        if let Some(group) = self.group() {
            f.write_str(group)?;
        }
        f.write_char('/')?;
        f.write_str(self.name())?;
        if let Some(namespace) = self.namespace() {
            f.write_char('.')?;
            f.write_str(namespace)?;
        }

        Ok(())
    }
}
impl ObjectRefBuilder {
    pub fn of_kind<K: Resource>(&mut self) -> &mut Self
    where
        K::DynamicType: 'static + Default,
    {
        let dynamic_type = K::DynamicType::default();
        let kind = K::kind(&dynamic_type);
        let version = K::version(&dynamic_type);
        let group = K::group(&dynamic_type);

        self.kind(kind).version(version);

        if !group.is_empty() {
            self.group(group.into_owned());
        } else {
            self.group(None);
        }

        self
    }

    pub fn from_object<K: Resource + ResourceExt>(&mut self, object: &K) -> &mut Self
    where
        K::DynamicType: 'static + Default,
    {
        self.of_kind::<K>()
            .namespace(object.namespace())
            .name(object.name().expect("Object must have a name"))
    }
}

#[derive(PartialEq, Debug)]
pub enum ObjectState<K> {
    Active(K),
    Deleted(K),
}

impl<K> ObjectState<K> {
    pub fn is_active(&self) -> bool {
        matches!(self, ObjectState::Active(_))
    }

    pub fn is_deleted(&self) -> bool {
        matches!(self, ObjectState::Deleted(_))
    }
}

#[derive(Getters, Clone, Default, PartialEq, Debug)]
pub struct Objects<K> {
    #[getset(get = "pub")]
    objects: BTreeMap<ObjectRef, Arc<ObjectState<K>>>,
}

impl<K> Objects<K> {
    pub fn set_active(&mut self, resource_ref: ObjectRef, resource: K) {
        self.objects
            .insert(resource_ref, Arc::new(ObjectState::Active(resource)));
    }

    pub fn set_deleted(&mut self, resource_ref: ObjectRef, resource: K) {
        self.objects
            .insert(resource_ref, Arc::new(ObjectState::Deleted(resource)));
    }

    pub fn is_active(&self, resource_ref: &ObjectRef) -> bool {
        self.objects
            .get(resource_ref)
            .map_or(false, |state| state.is_active())
    }

    pub fn filter_into<F>(&self, f: F) -> Objects<K>
    where
        F: Fn(&ObjectRef, &ObjectState<K>) -> bool,
    {
        let resources = BTreeMap::from_iter(
            self.objects
                .iter()
                .filter(|(r, s)| f(r, s))
                .map(|(r, s)| (r.clone(), s.clone())),
        );

        Self { objects: resources }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ObjectRef, &ObjectState<K>)> {
        self.objects.iter().map(|(r, s)| (r, s.as_ref()))
    }
}

#[derive(Builder, Getters, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[builder(setter(into))]
pub struct Zone {
    #[getset(get = "pub")]
    node: Option<String>,

    #[getset(get = "pub")]
    zone: Option<String>,
}

impl Zone {
    pub fn new_builder() -> ZoneBuilder {
        ZoneBuilder::default()
    }
}
