use crate::controllers::Ref;
use getset::Getters;
use std::collections::HashMap;
use std::fmt::Debug;
use thiserror::Error;

#[derive(Clone, PartialEq, Debug)]
pub enum ResourceState<K> {
    Active(K),
    Deleted(K),
}

#[derive(Getters, Default, Clone, PartialEq, Debug)]
pub struct Resources<K> {
    #[getset(get = "pub")]
    resources: HashMap<Ref, ResourceState<K>>,
}

impl<K> Resources<K> {
    pub fn insert(&mut self, key: Ref, value: ResourceState<K>) {
        self.resources.insert(key, value);
    }
}

#[derive(Error, Debug)]
pub enum ControllerError {}

#[macro_export]
macro_rules! spawn_controller {
    ($resource:ty, $join_set:ident, $client:ident) => {
        spawn_controller!($resource, $join_set, $client, Config::default())
    };
    ($resource:ty, $join_set:ident, $client:ident, $config:expr) => {{
        use crate::controllers::Ref;
        use crate::controllers::sources::controller::{ControllerError, ResourceState, Resources};
        use crate::sync::state::{Sender, channel};
        use futures::StreamExt;
        use kube::Api;
        use kube::runtime::Controller;
        use kube::runtime::controller::Action;
        use log::debug;
        use std::future::ready;
        use std::sync::Arc;
        use std::time::Duration;

        struct ControllerContext {
            client: Client,
            tx: Sender<Resources<$resource>>,
        }

        impl ControllerContext {
            fn new(client: Client, tx: Sender<Resources<$resource>>) -> Self {
                Self { client, tx }
            }
        }

        async fn reconcile(
            resource: Arc<$resource>,
            ctx: Arc<ControllerContext>,
        ) -> Result<Action, ControllerError> {
            let mut new_resources = ctx.tx.current();

            let metadata = &resource.metadata;

            let resource_ref = Ref::new_builder()
                .name(metadata.name.clone().expect("Resource must have a name"))
                .namespace(metadata.namespace.clone())
                .build()
                .expect("Failed to build Ref");

            let resource_state = match &metadata.deletion_timestamp {
                None => ResourceState::Active(resource.as_ref().clone()),
                _ => ResourceState::Deleted(resource.as_ref().clone()),
            };

            new_resources.insert(resource_ref, resource_state);

            ctx.tx.replace(new_resources);
            Ok(Action::requeue(Duration::from_secs(60)))
        }

        fn error_policy(
            _: Arc<$resource>,
            _: &ControllerError,
            _: Arc<ControllerContext>,
        ) -> Action {
            Action::requeue(Duration::from_secs(5))
        }

        let client = $client.clone();
        let resources = Api::<$resource>::all($client.clone());
        let config = $config.clone();
        let (tx, rx) = channel::<Resources<$resource>>(Resources::default());

        $join_set.spawn(async move {
            Controller::new(resources, config)
                .shutdown_on_signal()
                .run(
                    reconcile,
                    error_policy,
                    Arc::new(ControllerContext::new(client, tx)),
                )
                .filter_map(|x| async move { Some(x) })
                .for_each(|_| ready(()))
                .await;
        });

        rx
    }};
}
