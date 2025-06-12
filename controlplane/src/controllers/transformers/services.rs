use crate::controllers::resources::{ObjectRef, ObjectState, Objects, Zone};
use crate::controllers::transformers::http_routes::HttpRouteBackend;
use derive_builder::Builder;
use getset::Getters;
use itertools::Itertools;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kubera_core::select_continue;
use kubera_core::sync::signal::{channel, Receiver};
use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use tokio::task::JoinSet;

#[derive(Debug, Builder, Getters, Clone, Hash, PartialEq, Eq)]
pub struct Endpoints {
    #[getset(get = "pub")]
    zone_ref: Zone,

    #[getset(get = "pub")]
    addresses: Vec<IpAddr>,
}

#[derive(Debug, Builder, Getters, Clone, Hash, PartialEq, Eq)]
pub struct Backend {
    #[getset(get = "pub")]
    ref_: ObjectRef,

    #[getset(get = "pub")]
    endpoints: Vec<Endpoints>,
}

impl Backend {
    pub fn new_builder() -> BackendBuilder {
        BackendBuilder::default()
    }
}

pub fn collect_service_backends(
    join_set: &mut JoinSet<()>,
    http_route_backends: &Receiver<BTreeMap<ObjectRef, HttpRouteBackend>>,
    endpoint_slices: &Receiver<Objects<EndpointSlice>>,
) -> Receiver<BTreeMap<ObjectRef, Backend>> {
    let (tx, rx) = channel(BTreeMap::new());

    let mut http_route_backends = http_route_backends.clone();
    let mut endpoint_slices = endpoint_slices.clone();

    join_set.spawn(async move {
        loop {
            endpoint_slices
                .current()
                .iter()
                .filter_map(|(_, endpoint_slice)| {
                    if let ObjectState::Active(endpoint_slice) = endpoint_slice {
                        let metadata = &endpoint_slice.metadata;
                        let labels = metadata.labels.as_ref()?;
                        labels
                            .get("kubernetes.io/service-name")
                            .and_then(|service_name| {
                                ObjectRef::new_builder()
                                    .namespace(endpoint_slice.metadata.namespace.clone())
                                    .name(service_name)
                                    .build()
                                    .ok()
                            })
                        // .and_then(|service_ref| {
                        //     Some((service_ref, extract_ip_addrs(&endpoint_slice)))
                        // })
                    } else {
                        None
                    }
                });
            //.collect();

            // tx.replace(service_ips);

            select_continue!(http_route_backends.changed(), endpoint_slices.changed());
        }
    });

    rx
}

// fn extract_backends(service_ref: Ref, endpoint_slice: &EndpointSlice) -> Vec<HttpRouteBackend> {
//     let backend_ref = HttpRouteBackend::new_builder()
//         .service_ref(service_ref)
//         .build()
//         .expect("Failed to build ServiceBackendRef");
//     endpoint_slice
//         .endpoints
//         .iter()
//         .map(|endpoint| {
//             endpoint
//                 .addresses
//                 .iter()
//                 .flat_map(|a| match endpoint_slice.address_type.as_str() {
//                     "IPv4" => a.parse::<Ipv4Addr>().ok().map(|ip| IpAddr::from(ip)),
//                     "IPv6" => a.parse::<Ipv6Addr>().ok().map(|ip| IpAddr::from(ip)),
//                     _ => None,
//                 })
//         })
//         .dedup()
//         .collect()
// }
