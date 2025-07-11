use derive_builder::Builder;
use getset::Getters;
use gtmpl_value::Value;
use kube::runtime::reflector::Lookup;
use kube::{Resource, ResourceExt};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter, Write};
use std::sync::Arc;
use thiserror::Error;
use tracing::warn;

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

        if group.is_empty() {
            self.group(None);
        } else {
            self.group(group.into_owned());
        }

        self
    }

    pub fn for_object<K: Resource + ResourceExt>(&mut self, object: &K) -> &mut Self
    where
        K::DynamicType: 'static + Default,
    {
        self.of_kind::<K>();

        self.namespace = Some(object.namespace());
        self.name = object.name().map(|s| s.to_string());
        self
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

#[derive(Clone, PartialEq, Debug)]
pub struct Objects<K: Resource + ResourceExt> {
    by_ref: HashMap<ObjectRef, Arc<K>>,
    by_unique_id: HashMap<ObjectUniqueId, Arc<K>>,
}

impl<K: Resource + ResourceExt> Default for Objects<K> {
    fn default() -> Self {
        Self {
            by_ref: HashMap::new(),
            by_unique_id: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, Error)]
pub enum ObjectsError {
    #[error("Keys are invalid")]
    InvalidKeys,
}

impl<K: Resource + ResourceExt> Objects<K>
where
    K::DynamicType: 'static + Default,
{
    pub fn size(&self) -> usize {
        self.by_ref.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_ref.is_empty()
    }

    fn keys(object: &K) -> Option<(ObjectRef, ObjectUniqueId)> {
        let object_ref = ObjectRefBuilder::default()
            .for_object(object)
            .build()
            .ok()?;
        let uid = object.uid()?;
        let unique_id = ObjectUniqueId::new(uid);
        Some((object_ref, unique_id))
    }

    pub fn insert(&mut self, object: Arc<K>) -> Result<(), ObjectsError> {
        let (object_ref, unique_id) = Self::keys(&object).ok_or(ObjectsError::InvalidKeys)?;
        self.by_ref.insert(object_ref, object.clone());
        self.by_unique_id.insert(unique_id, object);
        Ok(())
    }

    pub fn remove(&mut self, object: &K) -> Result<(), ObjectsError> {
        let (object_ref, unique_id) = Self::keys(object).ok_or(ObjectsError::InvalidKeys)?;
        self.by_ref.remove(&object_ref);
        self.by_unique_id.remove(&unique_id);
        Ok(())
    }

    pub fn get_by_ref(&self, object_ref: &ObjectRef) -> Option<Arc<K>> {
        self.by_ref.get(object_ref).cloned()
    }

    pub fn get_by_unique_id(&self, unique_id: &ObjectUniqueId) -> Option<Arc<K>> {
        self.by_unique_id.get(unique_id).cloned()
    }

    pub fn contains_by_ref(&self, object_ref: &ObjectRef) -> bool {
        self.by_ref.contains_key(object_ref)
    }

    pub fn contains_by_unique_id(&self, unique_id: &ObjectUniqueId) -> bool {
        self.by_unique_id.contains_key(unique_id)
    }

    pub fn cloned_refs(&self) -> HashSet<ObjectRef> {
        self.by_ref.keys().cloned().collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = (ObjectRef, ObjectUniqueId, Arc<K>)> {
        self.by_ref.iter().filter_map(|(r, s)| match s.uid() {
            None => {
                warn!("Object {} does not have a UID, skipping", r);
                None
            }
            Some(uid) => Some((r.clone(), ObjectUniqueId::new(uid), s.clone())),
        })
    }
}

impl<K: Resource + ResourceExt> FromIterator<(ObjectRef, ObjectUniqueId, Arc<K>)> for Objects<K> {
    fn from_iter<I: IntoIterator<Item = (ObjectRef, ObjectUniqueId, Arc<K>)>>(iter: I) -> Self {
        let mut objects = Objects::default();
        for (object_ref, unique_id, state) in iter {
            objects.by_ref.insert(object_ref.clone(), state.clone());
            objects.by_unique_id.insert(unique_id.clone(), state);
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

#[derive(Clone, Debug)]
pub enum SyncObjectAction<T: Into<Value>, K: Resource + ResourceExt> {
    Upsert(ObjectRef, ObjectRef, T, Option<K>),
    Delete(ObjectRef),
}

impl<T: Into<Value>, K: Resource + ResourceExt> SyncObjectAction<T, K> {
    pub fn object_ref(&self) -> &ObjectRef {
        match self {
            SyncObjectAction::Upsert(object_ref, _, _, _)
            | SyncObjectAction::Delete(object_ref) => object_ref,
        }
    }
}
