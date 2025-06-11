#[macro_export]
macro_rules! spawn_controller {
    ($resource:ty, $join_set:ident, $client:ident) => {
        spawn_controller!($resource, $join_set, $client, Config::default())
    };
    ($resource:ty, $join_set:ident, $client:ident, $config:expr) => {{
        use crate::controllers::resources::{Ref, ResourceState, Resources};
        use futures::StreamExt;
        use kube::Api;
        use kube::runtime::Controller;
        use kube::runtime::controller::Action;
        use kubera_core::sync::signal::{Sender, channel};
        use std::fmt::Debug;
        use std::future::ready;
        use std::sync::Arc;
        use std::time::Duration;
        use thiserror::Error;
        use tracing::{debug, info};

        struct ControllerContext {
            tx: Sender<Resources<$resource>>,
        }

        impl ControllerContext {
            fn new(tx: Sender<Resources<$resource>>) -> Self {
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

            let resource = resource.as_ref().clone();

            match &metadata.deletion_timestamp {
                None => {
                    info!("Resource {:?} is active", resource_ref);
                    new_resources.set_active(resource_ref, resource)
                }
                _ => {
                    info!("Resource {:?} is deleted", resource_ref);
                    new_resources.set_deleted(resource_ref, resource)
                }
            };

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
        let (tx, rx) = channel::<Resources<$resource>>(Resources::default());

        debug!(
            "Spawning controller for resource: {}",
            stringify!($resource)
        );

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
