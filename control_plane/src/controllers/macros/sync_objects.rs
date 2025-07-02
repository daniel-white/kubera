#[macro_export]
macro_rules! sync_objects {
    ($join_set:ident, $object_type:ty, $kube_client:ident, $instance_role:ident, $template_value_type:ty, $template:ident) => {{
        use $crate::kubernetes::objects::{ObjectRef, SyncObjectAction, SyncObjectAction::*};
        use gtmpl::{Context, Template, gtmpl_fn, FuncError};
        use gtmpl_value::Value;
        use kube::{ Api, Resource, ResourceExt };
        use kube::api::{ Patch, ObjectMeta };
        use kubera_api::constants::{MANAGED_BY_LABEL, MANAGED_BY_VALUE, PART_OF_LABEL};
        use k8s_openapi::DeepMerge;
        use std::collections::BTreeMap;
        use tokio::select;
        use tokio::signal::ctrl_c;
        use tokio::sync::broadcast::{channel, error::RecvError};
        use tracing::{debug, info, trace, warn};
        use kubera_core::continue_on;

        let (tx, mut rx) = channel::<SyncObjectAction<$template_value_type, $object_type>>(1);

        const _: () = {
            fn assert_impl<T: Resource + ResourceExt>() {}
            fn assert_type_bounds() {
                assert_impl::<$object_type>();
            }
        };

        let kube_client: Receiver<Option<KubeClientCell>> = $kube_client.clone();
        let instance_role: Receiver<InstanceRole> = $instance_role.clone();

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

        fn render_object<V: Into<Value>>(
            template: &Template,
            object_ref: &ObjectRef,
            parent_ref: ObjectRef,
            value: V,
            object_overrides: Option<$object_type>
        ) -> $object_type {
            let context = Context::from(value);
            let yaml = template.render(&context).expect("Unable to render template");

            let mut object: $object_type = serde_yaml::from_str(&yaml)
                .expect("Unable to deserialize rendered template into object");

            let new_metadata = ObjectMeta {
                name: Some(object_ref.name().to_string()),
                namespace: object_ref.namespace().as_ref().cloned(),
                labels: Some(BTreeMap::from([
                    (MANAGED_BY_LABEL.to_string(), MANAGED_BY_VALUE.to_string()),
                    (PART_OF_LABEL.to_string(), parent_ref.name().to_string())
                ])),
                ..Default::default()
            };
            let mut existing_metadata = object.metadata;
            existing_metadata.merge_from(new_metadata);
            object.metadata = existing_metadata;

            if let Some(object_overrides) = object_overrides {
                object.merge_from(object_overrides);
            }

            object
        }

        debug!(
            "Spawning controller for writing {} objects",
            stringify!($object_type)
        );

        $join_set.spawn(async move {
            loop {
                let (kube_client, action) = select! {
                    action = rx.recv() => match action {
                        Ok(action) => {
                            if let Some(kube_client) = kube_client.current().as_ref() {
                                if instance_role.current().is_primary() {
                                    (kube_client.clone(), action)
                                } else {
                                    debug!("Skipping action for {} objects as instance is not primary", stringify!($object_type));
                                    continue;
                                }
                            } else {
                                continue_on!(kube_client.changed());
                            }
                        },
                        Err(RecvError::Lagged(_)) => {
                            debug!("Queue lagged for {} objects", stringify!($object_type));
                            continue;
                        }
                        Err(err) => {
                            debug!("Queue closed, shutting down controller for {} objects: {}", stringify!($object_type), err);
                            break;
                        }
                    },
                    _ = instance_role.changed() => {
                        continue;
                    },
                    _ = ctrl_c() => {
                        debug!("Received Ctrl+C, shutting down controller for {} objects", stringify!($object_type));
                        break;
                    }
                };

                let object_ref = action.object_ref();

                let api = Api::<$object_type>::namespaced(
                    kube_client.into(),
                    object_ref.namespace().as_ref().expect("Missing namespace"),
                );

                let exists = api.get_metadata(object_ref.name()).await.is_ok();

                trace!(
                    "Processing action: {:?} for object: {} {}",
                    action,
                    object_ref,
                    exists
                );

                match (action.clone(), exists) {
                    (Upsert(_, parent_ref, value, object_overrides), false) => {
                        let object = render_object(&template, &object_ref, parent_ref, value, object_overrides);
                        info!("Creating object: {}", object_ref);
                        api.create(&Default::default(), &object)
                            .await
                            .map_err(|e| warn!("Failed to create object: {}: {}", object_ref, e))
                            .ok();
                    }
                    (Upsert(_, parent_ref, value, object_overrides), true) => {
                        let object = render_object(&template, &object_ref, parent_ref, value, object_overrides);
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
