use crate::objects::Objects;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::ResourceExt;
use kubera_api::constants::{
    CONFIGMAP_ROLE_GATEWAY_CONFIG, CONFIGMAP_ROLE_LABEL, MANAGED_BY_LABEL, MANAGED_BY_VALUE,
};
use kubera_core::select_continue;
use kubera_core::sync::signal::{Receiver, channel};
use tokio::task::JoinSet;

pub fn filter_gateway_config_maps(
    join_set: &mut JoinSet<()>,
    config_maps: &Receiver<Objects<ConfigMap>>,
) -> Receiver<Objects<ConfigMap>> {
    let (tx, rx) = channel(Objects::default());

    let mut config_maps = config_maps.clone();

    join_set.spawn(async move {
        loop {
            let current = config_maps.current();
            let filtered: Objects<_> = current
                .iter()
                .filter(|(_, _, config_map)| {
                    let config_map = config_map.as_ref();

                    let is_managed_by = config_map
                        .labels()
                        .get(MANAGED_BY_LABEL)
                        .map(|l| l.as_str() == MANAGED_BY_VALUE)
                        .unwrap_or(false);
                    let is_gateway_config = config_map
                        .labels()
                        .get(CONFIGMAP_ROLE_LABEL)
                        .map(|l| l.as_str() == CONFIGMAP_ROLE_GATEWAY_CONFIG)
                        .unwrap_or(false);

                    return is_managed_by && is_gateway_config;
                })
                .collect();

            tx.replace(filtered);

            select_continue!(config_maps.changed());
        }
    });

    rx
}
