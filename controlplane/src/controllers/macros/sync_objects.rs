#[macro_export]
macro_rules! sync_objects {
    ($join_set:ident, $object_type:ty, $client:ident, $template_value_type:ty, $template:ident) => {{
        use $crate::objects::{ObjectRef, SyncObjectAction, SyncObjectAction::*};
        use gtmpl::{Context, Template, gtmpl_fn, FuncError};
        use gtmpl_value::Value;
        use kube::{ Api, Resource, ResourceExt };
        use kube::api::{ Patch, ObjectMeta };
        use k8s_openapi::DeepMerge;
        use tokio::select;
        use tokio::sync::broadcast::{channel, error::RecvError};
        use tokio::signal::ctrl_c;
        use tracing::{debug, info, warn, trace};
        use std::collections::BTreeMap;
        use kubera_api::constants::{MANAGED_BY_LABEL, MANAGED_BY_VALUE};

        let (tx, mut rx) = channel::<SyncObjectAction<$template_value_type>>(1);

        const _: () = {
            fn assert_impl<T: Resource + ResourceExt>() {}
            fn assert_type_bounds() {
                assert_impl::<$object_type>();
            }
        };

        let client = $client.clone();

        gtmpl_fn!(
            fn quote(s: String) -> Result<String, FuncError> {
                Ok(format!("\"{}\"", s.replace("\"", "\\\"")))
            }
        );

        let template: Template = {
            use sprig::{defaults::default, strings::{indent, nindent}};

            let mut template = Template::default();
            template.add_func("default", default);
            template.add_func("indent", indent);
            template.add_func("nindent", nindent);
            template.add_func("quote", quote);
            template.parse($template).expect("Unable to parse template");

            template
        };

        fn render_object<V: Into<Value>>(template: &Template, object_ref: &ObjectRef, value: V) -> $object_type {
            let context = Context::from(value);
            let yaml = template.render(&context).expect("Unable to render template");

            let mut object: $object_type = serde_yaml::from_str(&yaml)
                .expect("Unable to deserialize rendered template into object");

            let new_metadata = ObjectMeta {
                name: Some(object_ref.name().to_string()),
                namespace: object_ref.namespace().as_ref().cloned(),
                labels: Some(BTreeMap::from([
                    (MANAGED_BY_LABEL.to_string(), MANAGED_BY_VALUE.to_string())
                ])),
                ..Default::default()
            };
            let mut existing_metadata = object.metadata;
            existing_metadata.merge_from(new_metadata);
            object.metadata = existing_metadata;

            object
        }

        debug!(
            "Spawning controller for writing {} objects",
            stringify!($object_type)
        );

        $join_set.spawn(async move {
            loop {
                let action = select! {
                    action = rx.recv() => match action {
                        Ok(action) => action,
                        Err(RecvError::Lagged(_)) => {
                            debug!("Queue lagged for {} objects", stringify!($object_type));
                            continue;
                        }
                        Err(err) => {
                            debug!("Queue closed, shutting down controller for {} objects: {}", stringify!($object_type), err);
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
                    client.clone(),
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
                    (Upsert(_, value), false) => {
                        let object = render_object(&template, &object_ref, value.clone());
                        info!("Creating object: {}", object_ref);
                        api.create(&Default::default(), &object)
                            .await
                            .map_err(|e| warn!("Failed to create object: {}: {}", object_ref, e))
                            .ok();
                    }
                    (Upsert(_, value), true) => {
                        let object = render_object(&template, &object_ref, value.clone());
                        info!("Patching object: {}", object_ref);
                        api.patch(object_ref.name(), &Default::default(), &Patch::Strategic(object))
                            .await
                            .map_err(|e| warn!("Failed to patch object: {}: {}", object_ref, e))
                            .ok();
                    }
                    (Delete(_), true) => {
                        info!("Deleting object: {}", object_ref);
                        api.delete(object_ref.name(), &Default::default())
                            .await
                            .map_err(|e| warn!("Failed to delete object: {}: {}", object_ref, e))
                            .ok();
                    }
                    _ => info!("Skipping action for object: {}", &object_ref),
                };
            }
        });

        tx
    }};
}
