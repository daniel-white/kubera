#[macro_export]
macro_rules! sync_objects {
    ($options:ident, $task_builder:ident, $object_type:ty, $kube_client_rx:ident, $instance_role_rx:ident, $template_value_type:ty, $template:ident) => {{
        use $crate::kubernetes::objects::{Objects, ObjectRef, SyncObjectAction, SyncObjectAction::*};
        use gtmpl::{Context, Template, gtmpl_fn, FuncError};
        use kube::Api;
        use kube::api::{ Patch, ObjectMeta };
        use kube::Client;
        use vg_api::constants::{MANAGED_BY_LABEL, MANAGED_BY_VALUE, PART_OF_LABEL, MANAGED_BY_LABEL_QUERY};
        use k8s_openapi::DeepMerge;
        use std::collections::BTreeMap;
        use tokio::select;
        use tokio::signal::ctrl_c;
        use tokio::sync::mpsc::unbounded_channel;
        use tracing::{debug, info, trace, warn};
        use vg_core::{continue_after, continue_on};
        use vg_core::sync::signal::{signal, Receiver};
        use std::collections::HashSet;
        use $crate::options::Options;
        use vg_core::task::Builder as TaskBuilder;

        let (tx, mut rx) = unbounded_channel::<SyncObjectAction<$template_value_type, $object_type>>();

        let options: Arc<Options> = $options.clone();
        let kube_client_rx: Receiver<KubeClientCell> = $kube_client_rx.clone();
        let instance_role_rx: Receiver<InstanceRole> = $instance_role_rx.clone();
        let task_builder: &TaskBuilder = $task_builder;

        let current_object_refs_rx = {
            let (tx, rx) = signal();
            let current_objects_rx: Receiver<Objects<$object_type>> = watch_objects!(
                $options,
                task_builder,
                $object_type,
                kube_client_rx,
                Some(MANAGED_BY_LABEL_QUERY)
            );

            task_builder
                .new_task(concat!("current_object_refs_", stringify!($object_type)))
                .spawn(async move {
                    loop {
                        if let Some(current_objects) = current_objects_rx.get().await {
                            debug!("Reconciling {} object refs", stringify!($object_type));

                            let object_refs: HashSet<_> = current_objects
                                .iter()
                                .filter_map(|(object_ref, _, object)| {
                                    match object.metadata.deletion_timestamp {
                                        Some(_) => None,
                                        None => Some(object_ref.clone()),
                                    }
                                })
                                .collect();

                            tx.set(object_refs).await;
                        } else {
                            debug!("Signal for {} hasn't signaled yet for reconciling object refs", stringify!($object_type));
                        }

                        continue_after!(options.auto_cycle_duration(), current_objects_rx.changed());
                    }
                });

            rx
        };

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

        fn render_object(
            template: &Template,
            object_ref: &ObjectRef,
            parent_ref: ObjectRef,
            value: $template_value_type,
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
                object.merge_from(object_overrides.clone());
            }

            object
        }

        async fn apply_action(kube_client: Client, template: &Template, action: SyncObjectAction<$template_value_type, $object_type>) {
            let object_ref = action.object_ref();
            let api = Api::<$object_type>::namespaced(
                kube_client,
                object_ref.namespace().as_ref().expect("Missing namespace"),
            );

            let exists = api.get_metadata(object_ref.name()).await.is_ok();

            trace!(
                "Processing action: {:?} for object: {} {}",
                action,
                object_ref,
                exists
            );

             match (action, exists) {
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

        debug!(
            "Spawning controller for writing {} objects",
            stringify!($object_type)
        );

        task_builder
            .new_task(concat!("current_objects_", stringify!($object_type)))
            .spawn(async move {
                loop {
                    match (kube_client_rx.get().await, instance_role_rx.get().await) {
                        (Some(kube_client), Some(instance_role)) => {
                            let _ = select! {
                                action = rx.recv() => match action {
                                    Some(action) if instance_role.is_primary() => apply_action(kube_client.clone().into(), &template, action).await,
                                    Some(action) => {
                                        debug!("Skipping action {:?} for {} objects as instance is not primary", action, stringify!($object_type));
                                    }
                                    None => {
                                        debug!("Channel closed, shutting down controller for {} objects", stringify!($object_type));
                                        break;
                                    }
                                },
                                _ = instance_role_rx.changed() => {
                                    continue;
                                },
                                _ = ctrl_c() => {
                                    debug!("Received Ctrl+C, shutting down controller for {} objects", stringify!($object_type));
                                    break;
                                }
                            };
                        }
                        (None, _) => {
                            debug!("Kube client is not available, unable to apply changes to {} objects", stringify!($object_type));
                        }
                        (_, None) => {
                            debug!("Instance role is not available, unable to apply changes to {} objects", stringify!($object_type));
                        }
                    }

                    continue_on!(kube_client_rx.changed(), instance_role_rx.changed());
                }
            });

        (tx, current_object_refs_rx)
    }}
}
