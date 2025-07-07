#[macro_export]
macro_rules! watch_objects {
    ($options:ident, $join_set:ident, $object_type:ty, $kube_client:ident) => {{
        #[allow(unused_imports)]
        use kube::runtime::watcher::Config;

        watch_objects!($options, $join_set, $object_type, $kube_client, Config::default())
    }};
    ($options:ident, $join_set:ident, $object_type:ty, $kube_client:ident, $config:expr) => {{
        use futures::StreamExt;
        use kube::Api;
        use kube::runtime::Controller;
        use kube::runtime::controller::Action;
        use kubera_core::sync::signal::{Sender, channel};
        use std::fmt::Debug;
        use std::future::ready;
        use std::sync::Arc;
        use thiserror::Error;
        use tracing::instrument;
        use tracing::debug;
        use kubera_core::continue_on;
        use $crate::kubernetes::objects::Objects;
        use $crate::Options;

        struct ControllerContext {
            options: Arc<Options>,
            tx: Sender<Objects<$object_type>>,
        }

        #[derive(Error, Debug)]
        enum ControllerError {}

        #[instrument(skip(object, ctx), level = "debug")]
        async fn reconcile(
            object: Arc<$object_type>,
            ctx: Arc<ControllerContext>,
        ) -> Result<Action, ControllerError> {
            let objects = ctx.tx.current();
            let mut new_objects = objects.as_ref().clone();

            let metadata = &object.metadata;

            if metadata.deletion_timestamp.is_none() {
                debug!(
                    "reconciled object; object.namespace={:?} object.name={:?} object.state=active",
                    metadata.namespace, metadata.name
                );
                new_objects.insert(object);
            } else {
                debug!(
                    "reconciled object; object.namespace={:?} object.name={:?} object.state=deleted",
                    metadata.namespace, metadata.name
                );
                new_objects.remove(object);
            }

            ctx.tx.replace(new_objects);

            Ok(Action::requeue(ctx.options.controller_requeue_duration()))
        }

        fn error_policy(
            _: Arc<$object_type>,
            _: &ControllerError,
            ctx: Arc<ControllerContext>,
        ) -> Action {
            Action::requeue(ctx.options.controller_error_requeue_duration())
        }

        let options: Arc<Options> = $options.clone();
        let kube_client: Receiver<Option<KubeClientCell>> = $kube_client.clone();
        let config: Config = $config.clone();
        let (tx, rx) = channel::<Objects<$object_type>>(Objects::default());

        debug!(
            "Spawning controller for watching {} objects",
            stringify!($object_type)
        );

        $join_set.spawn(async move {
            let controller_context = Arc::new(ControllerContext{
                options,
                tx,
            });
            loop {
                if let Some(kube_client) = kube_client.current().as_ref() {
                    let object_api = Api::<$object_type>::all(kube_client.clone().into());
                    Controller::new(object_api, config.clone())
                        .shutdown_on_signal()
                        .run(
                            reconcile,
                            error_policy,
                            controller_context.clone(),
                        )
                        .filter_map(|x| async move { Some(x) })
                        .for_each(|_| ready(()))
                        .await;
                } else {
                    continue_on!(kube_client.changed());
                }
            }
        });

        rx
    }};
}
