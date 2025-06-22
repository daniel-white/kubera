#[macro_export]
macro_rules! sync_objects {
    ($object_type:ty, $client:ident, $queue:ident, $template_value:ty, $template:literal) => {{
        use gtmpl::Template;
        use kube::Api;
        use tokio::spawn;
        use tokio::sync::broadcast::Receiver;
        use tracing::{debug, info, warn};
        use $crate::objects::{ObjectRef, SyncObjectAction, SyncObjectAction::*};
        use tokio::signal::ctrl_c;
        use tokio::select;

        let client = $client.clone();
        let mut queue_recv: Receiver<SyncObjectAction<$template_value>> = $queue.subscribe();

        let template: Template = {
            let mut template = Template::default();
            template.parse($template).expect("Unable to parse template");
            template
        };

        debug!(
            "Spawning controller for writing {} objects",
            stringify!($object_type)
        );

        spawn(async move {
            loop {
                let action = select! {
                    action = queue_recv.recv() => match action {
                        Ok(action) => action,
                        Err(_) => {
                            debug!("Queue closed, shutting down controller for {} objects", stringify!($object_type));
                            break;
                        }
                    },
                    _ = ctrl_c() => {
                        debug!("Received Ctrl+C, shutting down controller for {} objects", stringify!($object_type));
                        break;
                    }
                };

                let object_ref = action.object_ref();

                let api = Api::<$object_type>::namespaced(
                    $client.clone(),
                    object_ref.namespace().as_ref().expect("Missing namespace"),
                );

                let exists = api.get_metadata(object_ref.name()).await.is_ok();

                trace!(
                    "Processing action: {:?} for object: {} {}",
                    action,
                    object_ref,
                    exists
                );

                match (&action, exists) {
                    (Upsert((object_ref, value)), false) => {
                    }
                    (Upsert((object_ref, value)), true) => {
                    }
                    (SyncObjectAction::Delete(object_ref), true) => {
                        info!("Deleting object: {}", object_ref);
                        api.delete(object_ref.name(), &Default::default()).await
                            .map_err(|e| warn!("Failed to delete object: {}: {}", object_ref, e))
                            .ok();
                    }
                    _ => info!("Skipping action for object: {}", object_ref),
                };
            }
        });
    }};
}
