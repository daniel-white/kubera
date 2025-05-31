use crate::controllers::Ref;
use getset::Getters;
use std::collections::BTreeMap;
use std::fmt::Debug;

#[derive(Clone, PartialEq, Debug)]
pub enum SourceResourceState<K> {
    Active(K),
    Deleted(K),
}

#[derive(Getters, Default, Clone, PartialEq, Debug)]
pub struct SourceResources<K> {
    #[getset(get = "pub")]
    resources: BTreeMap<Ref, SourceResourceState<K>>,
}

impl<K> SourceResources<K> {
    pub fn set(&mut self, key: Ref, value: SourceResourceState<K>) {
        self.resources.insert(key, value);
    }
}

#[macro_export]
macro_rules! spawn_controller {
    ($resource:ty, $join_set:ident, $client:ident) => {
        spawn_controller!($resource, $join_set, $client, Config::default())
    };
    ($resource:ty, $join_set:ident, $client:ident, $config:expr) => {{
        use crate::controllers::Ref;
        use crate::controllers::source_controller::{SourceResourceState, SourceResources};
        use crate::sync::state::{Sender, channel};
        use futures::StreamExt;
        use kube::Api;
        use kube::runtime::Controller;
        use kube::runtime::controller::Action;
        use std::fmt::Debug;
        use std::future::ready;
        use std::sync::Arc;
        use std::time::Duration;
        use thiserror::Error;

        struct ControllerContext {
            tx: Sender<SourceResources<$resource>>,
        }

        impl ControllerContext {
            fn new(tx: Sender<SourceResources<$resource>>) -> Self {
                Self { tx }
            }
        }

        #[derive(Error, Debug)]
        enum ControllerError {}

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
                None => SourceResourceState::Active(resource.as_ref().clone()),
                _ => SourceResourceState::Deleted(resource.as_ref().clone()),
            };

            new_resources.set(resource_ref, resource_state);

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
        let config = $config.clone();
        let (tx, rx) = channel::<SourceResources<$resource>>(SourceResources::default());

        $join_set.spawn(async move {
            let resources = Api::<$resource>::all(client);
            Controller::new(resources, config)
                .shutdown_on_signal()
                .run(
                    reconcile,
                    error_policy,
                    Arc::new(ControllerContext::new(tx)),
                )
                .filter_map(|x| async move { Some(x) })
                .for_each(|_| ready(()))
                .await;
        });

        rx
    }};
}
