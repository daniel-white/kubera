#[macro_export]
macro_rules! watch_objects {
    ($object_type:ty, $client:ident) => {
        watch_objects!($object_type, $client, Config::default())
    };
    ($object_type:ty, $client:ident, $config:expr) => {{
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
        use tokio::spawn;
        use tracing::instrument;
        use tracing::{debug, info};
        use $crate::objects::{ObjectRef, Objects};

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

        #[instrument(skip(object, ctx))]
        async fn reconcile(
            object: Arc<$object_type>,
            ctx: Arc<ControllerContext>,
        ) -> Result<Action, ControllerError> {
            let objects = ctx.tx.current();
            let mut new_objects = objects.as_ref().clone();

            let metadata = &object.metadata;

            let ref_ = ObjectRef::new_builder()
                .from_object(object.as_ref())
                .build()
                .expect("Failed to build ObjectRef");

            let object = object.as_ref().clone();

            match &metadata.deletion_timestamp {
                None => {
                    info!("reconciled object; object.ref={} object.state=active", ref_);
                    new_objects.set_active(ref_, object)
                }
                _ => {
                    info!(
                        "reconciled object; object.ref={} object.state=deleted",
                        ref_
                    );
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
            "Spawning controller for watching {} objects",
            stringify!($object_type)
        );

        spawn(async move {
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
