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

#[derive(PartialEq, Debug)]
pub enum ObjectState<K: Resource + ResourceExt> {
    Active(K),
    Deleted(K),
}

impl<K: Resource + ResourceExt> ObjectState<K> {
    pub fn is_active(&self) -> bool {
        matches!(self, ObjectState::Active(_))
    }

    pub fn is_deleted(&self) -> bool {
        matches!(self, ObjectState::Deleted(_))
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Objects<K: Resource + ResourceExt> {
    objects_by_ref: HashMap<ObjectRef, Arc<ObjectState<K>>>,
    objects_by_unique_id: HashMap<ObjectUniqueId, Arc<ObjectState<K>>>,
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
        let object = Arc::new(ObjectState::Active(object));
        self.objects_by_ref.insert(object_ref, object.clone());
        self.objects_by_unique_id.insert(uid, object);
    }

    pub fn set_deleted(&mut self, object_ref: ObjectRef, object: K) {
        let uid = ObjectUniqueId::new(object.uid().unwrap());
        let object = Arc::new(ObjectState::Deleted(object));
        self.objects_by_ref.insert(object_ref, object.clone());
        self.objects_by_unique_id.insert(uid, object);
    }

    pub fn get_by_ref(&self, object_ref: &ObjectRef) -> Option<Arc<ObjectState<K>>> {
        self.objects_by_ref.get(object_ref).cloned()
    }

    pub fn get_by_unique_id(&self, unique_id: &ObjectUniqueId) -> Option<Arc<ObjectState<K>>> {
        self.objects_by_unique_id.get(unique_id).cloned()
    }

    pub fn iter(&self) -> impl Iterator<Item = (ObjectRef, ObjectUniqueId, Arc<ObjectState<K>>)> {
        self.objects_by_ref.iter().map(|(r, s)| {
            let uid = match s.as_ref() {
                ObjectState::Active(o) => ObjectUniqueId::new(o.uid().unwrap()),
                ObjectState::Deleted(o) => ObjectUniqueId::new(o.uid().unwrap()),
            };

            (r.clone(), uid, s.clone())
        })
    }
}

impl<K: Resource + ResourceExt> FromIterator<(ObjectRef, ObjectUniqueId, Arc<ObjectState<K>>)>
    for Objects<K>
{
    fn from_iter<I: IntoIterator<Item = (ObjectRef, ObjectUniqueId, Arc<ObjectState<K>>)>>(
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
