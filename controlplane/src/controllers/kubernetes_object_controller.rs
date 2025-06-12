#[macro_export]
macro_rules! spawn_controller {
    ($object_type:ty, $join_set:ident, $client:ident) => {
        spawn_controller!($object_type, $join_set, $client, Config::default())
    };
    ($object_type:ty, $join_set:ident, $client:ident, $config:expr) => {{
        use crate::controllers::resources::{ObjectRef, Objects};
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
            tx: Sender<Objects<$object_type>>,
        }

        impl ControllerContext {
            fn new(tx: Sender<Objects<$object_type>>) -> Self {
                Self { tx }
            }
        }

        #[derive(Error, Debug)]
        enum ControllerError {}

        async fn reconcile(
            object: Arc<$object_type>,
            ctx: Arc<ControllerContext>,
        ) -> Result<Action, ControllerError> {
            let mut new_objects = ctx.tx.current();

            let metadata = &object.metadata;

            let ref_ = ObjectRef::new_builder()
                .from_object(object.as_ref())
                .build()
                .expect("Failed to build Ref");

            let object = object.as_ref().clone();

            match &metadata.deletion_timestamp {
                None => {
                    info!("reconciled object; object.ref={} object.state=active", ref_);
                    new_objects.set_active(ref_, object)
                }
                _ => {
                    info!("reconciled object; object.ref={} object.state=deleted", ref_);
                    new_objects.set_deleted(ref_, object)
                }
            };

            ctx.tx.replace(new_objects);

            Ok(Action::requeue(Duration::from_secs(60)))
        }

        fn error_policy(
            _: Arc<$object_type>,
            _: &ControllerError,
            _: Arc<ControllerContext>,
        ) -> Action {
            Action::requeue(Duration::from_secs(5))
        }

        let client = $client.clone();
        let config = $config.clone();
        let (tx, rx) = channel::<Objects<$object_type>>(Objects::default());

        debug!(
            "Spawning controller for object: {}",
            stringify!($object_type)
        );

        $join_set.spawn(async move {
            let object_api = Api::<$object_type>::all(client);
            Controller::new(object_api, config)
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
