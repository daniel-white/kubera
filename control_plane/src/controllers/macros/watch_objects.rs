#[macro_export]
macro_rules! watch_objects {
    ($options:ident, $task_builder:ident, $object_type:ty, $kube_client_rx:ident) => {{
        #[allow(unused_imports)]
        use kube::runtime::watcher::Config;

        watch_objects!($options, $task_builder, $object_type, $kube_client_rx, Config::default())
    }};
    ($options:ident, $task_builder:ident, $object_type:ty, $kube_client_rx:ident, $config:expr) => {{
        use futures::StreamExt;
        use kube::Api;
        use kube::runtime::Controller;
        use kube::runtime::controller::Action;
        use kubera_core::sync::signal::{Sender, signal};
        use std::fmt::Debug;
        use std::future::ready;
        use std::sync::Arc;
        use thiserror::Error;
        use tracing::instrument;
        use tracing::{debug, warn};
        use kubera_core::continue_on;
        use tokio::select;
        use tokio::signal::ctrl_c;
        use kubera_core::task::Builder as TaskBuilder;
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
            let mut objects = match ctx.tx.get() {
                Some(objects) => (*objects).clone(),
                None => Objects::default()
            };

            let metadata = &object.metadata;

            if metadata.deletion_timestamp.is_none() {
                debug!(
                    "reconciled object; object.namespace={:?} object.name={:?} object.state=active",
                    metadata.namespace, metadata.name
                );
                if let Err(err) = objects.insert(object) {
                    warn!(
                        "Failed to insert object into objects set: {}",
                        err
                    );
                }
            } else {
                debug!(
                    "reconciled object; object.namespace={:?} object.name={:?} object.state=deleted",
                    metadata.namespace, metadata.name
                );
                if let Err(err) = objects.remove(&object) {
                    warn!(
                        "Failed to remove object from objects set: {}",
                        err
                    );
                }
            }

            ctx.tx.set(objects);

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
        let kube_client_rx: Receiver<KubeClientCell> = $kube_client_rx.clone();
        let config: Config = $config.clone();
        let task_builder: &TaskBuilder = $task_builder;
        let (tx, rx) = signal();

        debug!(
            "Spawning controller for watching {} objects",
            stringify!($object_type)
        );

        task_builder
            .new_task(concat!("watch_objects_", stringify!($object_type)))
            .spawn(async move
            {
                let controller_context = Arc::new(ControllerContext{
                    options,
                    tx,
                });
                loop {
                    if let Some(kube_client) = kube_client_rx.get() {
                        debug!(
                            "Starting controller for watching {} objects",
                            stringify!($object_type)
                        );
                        let object_api = Api::<$object_type>::all(kube_client.clone().into());

                        let controller = Controller::new(object_api, config.clone())
                            .shutdown_on_signal()
                            .run(
                                reconcile,
                                error_policy,
                                controller_context.clone(),
                            )
                            .filter_map(|x| async move { Some(x) })
                            .for_each(|_| ready(()));

                        select! {
                            _ = controller => {
                                debug!(
                                    "Controller for watching {} objects has stopped",
                                    stringify!($object_type)
                                );
                                break;
                            },
                            _ = ctrl_c() => {
                                debug!(
                                    "Received Ctrl+C signal, stopping controller for watching {} objects",
                                    stringify!($object_type)
                                );
                                break;
                            },
                            _ = kube_client_rx.changed() => {
                                debug!(
                                    "Kube client changed, restarting controller for watching {} objects",
                                    stringify!($object_type)
                                );
                                continue;
                            }
                        }
                    }

                    continue_on!(kube_client_rx.changed());
                }
            });

        rx
    }};
}
