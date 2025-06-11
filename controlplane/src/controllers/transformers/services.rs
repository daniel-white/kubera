use crate::controllers::resources::ServiceBackend;
use crate::controllers::resources::{Ref, ResourceState, Resources};
use itertools::Itertools;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kubera_core::select_continue;
use kubera_core::sync::signal::{Receiver, channel};
use multimap::MultiMap;
use std::collections::{BTreeMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use tokio::task::JoinSet;

pub fn collect_service_endpoint_ips(
    join_set: &mut JoinSet<()>,
    backend_services: &Receiver<BTreeMap<Ref, ServiceBackend>>,
    endpoint_slices: &Receiver<Resources<EndpointSlice>>,
) -> Receiver<MultiMap<Ref, IpAddr>> {
    let (tx, rx) = channel(MultiMap::new());

    let mut backend_services = backend_services.clone();
    let mut endpoint_slices = endpoint_slices.clone();

    join_set.spawn(async move {
        loop {
            let ip_addrs: HashSet<_> = endpoint_slices
                .current()
                .iter()
                .filter_map(|(_, endpoint_slice)| {
                    if let ResourceState::Active(endpoint_slice) = endpoint_slice {
                        let metadata = &endpoint_slice.metadata;
                        let labels = metadata.labels.as_ref()?;
                        labels
                            .get("kubernetes.io/service-name:")
                            .and_then(|service_name| {
                                Ref::new_builder()
                                    .namespace(endpoint_slice.metadata.namespace.clone())
                                    .name(service_name)
                                    .build()
                                    .ok()
                            })
                            .and_then(|service_ref| {
                                Some((service_ref, extract_ip_addrs(&endpoint_slice)))
                            })
                    } else {
                        None
                    }
                })
                .collect();

            let mut service_ips = MultiMap::new();
            for (service_ref, ips) in ip_addrs {
                service_ips.insert_many(service_ref, ips);
            }

            tx.replace(service_ips);

            select_continue!(backend_services.changed(), endpoint_slices.changed());
        }
    });

    rx
}

fn extract_ip_addrs(endpoint_slice: &EndpointSlice) -> Vec<IpAddr> {
    endpoint_slice
        .endpoints
        .iter()
        .flat_map(|endpoint| {
            endpoint
                .addresses
                .iter()
                .flat_map(|a| match endpoint_slice.address_type.as_str() {
                    "IPv4" => a.parse::<Ipv4Addr>().ok().map(|ip| IpAddr::from(ip)),
                    "IPv6" => a.parse::<Ipv6Addr>().ok().map(|ip| IpAddr::from(ip)),
                    _ => None,
                })
        })
        .dedup()
        .collect()
}
