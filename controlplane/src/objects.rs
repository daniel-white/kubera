use derive_builder::Builder;
use getset::Getters;
use kube::runtime::reflector::Lookup;
use kube::{Resource, ResourceExt};
use std::collections::HashMap;
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

#[derive(Clone, Debug, Getters, PartialEq, Eq, Hash)]
pub struct ObjectUniqueId(#[getset(get = "pub")] String);

impl ObjectUniqueId {
    pub fn new<S: Into<String>>(id: S) -> Self {
        ObjectUniqueId(id.into())
    }
}

impl Display for ObjectUniqueId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(PartialEq, Debug)]
pub enum ObjectState<K: Resource + ResourceExt> {
    Active(Arc<K>),
    Deleted(Arc<K>),
}

impl<K: Resource + ResourceExt> ObjectState<K> {
    pub fn is_active(&self) -> bool {
        matches!(self, ObjectState::Active(_))
    }

    pub fn is_deleted(&self) -> bool {
        matches!(self, ObjectState::Deleted(_))
    }

    pub fn cloned(&self) -> ObjectState<K> {
        match self {
            ObjectState::Active(object) => ObjectState::Active(object.clone()),
            ObjectState::Deleted(object) => ObjectState::Deleted(object.clone()),
        }
    }
}

impl<K: Resource + ResourceExt> AsRef<K> for ObjectState<K> {
    fn as_ref(&self) -> &K {
        match self {
            ObjectState::Active(object) => object.as_ref(),
            ObjectState::Deleted(object) => object.as_ref(),
        }
    }
}

impl<K: Resource + ResourceExt> Clone for ObjectState<K> {
    fn clone(&self) -> Self {
        match self {
            ObjectState::Active(object) => ObjectState::Active(object.clone()),
            ObjectState::Deleted(object) => ObjectState::Deleted(object.clone()),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Objects<K: Resource + ResourceExt> {
    objects_by_ref: HashMap<ObjectRef, ObjectState<K>>,
    objects_by_unique_id: HashMap<ObjectUniqueId, ObjectState<K>>,
}

impl<K: Resource + ResourceExt> Default for Objects<K> {
    fn default() -> Self {
        Self {
            objects_by_ref: HashMap::new(),
            objects_by_unique_id: HashMap::new(),
        }
    }
}

impl<K: Resource + ResourceExt> Objects<K> {
    pub fn set_active(&mut self, object_ref: ObjectRef, object: K) {
        let uid = ObjectUniqueId::new(object.uid().unwrap());
        let object = ObjectState::Active(Arc::new(object));
        self.objects_by_ref.insert(object_ref, object.cloned());
        self.objects_by_unique_id.insert(uid, object);
    }

    pub fn set_deleted(&mut self, object_ref: ObjectRef, object: K) {
        let uid = ObjectUniqueId::new(object.uid().unwrap());
        let object = ObjectState::Deleted(Arc::new(object));
        self.objects_by_ref.insert(object_ref, object.cloned());
        self.objects_by_unique_id.insert(uid, object);
    }

    pub fn get_by_ref(&self, object_ref: &ObjectRef) -> Option<ObjectState<K>> {
        self.objects_by_ref.get(object_ref).cloned()
    }

    pub fn get_by_unique_id(&self, unique_id: &ObjectUniqueId) -> Option<ObjectState<K>> {
        self.objects_by_unique_id.get(unique_id).cloned()
    }

    pub fn iter(&self) -> impl Iterator<Item = (ObjectRef, ObjectUniqueId, ObjectState<K>)> {
        self.objects_by_ref.iter().map(|(r, s)| {
            let uid = ObjectUniqueId::new(s.as_ref().uid().unwrap());
            (r.clone(), uid, s.cloned())
        })
    }
}

impl<K: Resource + ResourceExt> FromIterator<(ObjectRef, ObjectUniqueId, ObjectState<K>)>
    for Objects<K>
{
    fn from_iter<I: IntoIterator<Item = (ObjectRef, ObjectUniqueId, ObjectState<K>)>>(
        iter: I,
    ) -> Self {
        let mut objects = Objects::default();
        for (object_ref, unique_id, state) in iter {
            objects
                .objects_by_ref
                .insert(object_ref.clone(), state.clone());
            objects
                .objects_by_unique_id
                .insert(unique_id.clone(), state);
        }
        objects
    }
}

#[derive(Builder, Getters, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[builder(setter(into))]
pub struct TopologyLocation {
    #[getset(get = "pub")]
    node: Option<String>,

    #[getset(get = "pub")]
    zone: Option<String>,
}

impl TopologyLocation {
    pub fn new_builder() -> TopologyLocationBuilder {
        TopologyLocationBuilder::default()
    }
}
