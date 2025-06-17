use crate::controllers::transformers::http_routes::HttpRouteBackend;
use crate::objects::{ObjectRef, ObjectState, Objects, Zone};
use derive_builder::Builder;
use getset::Getters;
use k8s_openapi::api::core::v1::Service;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kubera_core::select_continue;
use kubera_core::sync::signal::{Receiver, channel};
use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use tokio::task::JoinSet;
use tracing::debug;

#[derive(Debug, Builder, Getters, Clone, Hash, PartialEq, Eq)]
pub struct Endpoints {
    #[getset(get = "pub")]
    zone_ref: Zone,

    #[getset(get = "pub")]
    addresses: Vec<IpAddr>,
}

impl Endpoints {
    pub fn new_builder() -> EndpointsBuilder {
        EndpointsBuilder::default()
    }
}

#[derive(Debug, Builder, Getters, Clone, Hash, PartialEq, Eq)]
pub struct Backend {
    #[getset(get = "pub")]
    object_ref: ObjectRef,

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
            let current_endpoint_slices = endpoint_slices.current();
            let current_http_route_backends = http_route_backends.current();
            let endpoint_slices_by_service: BTreeMap<_, _> = current_endpoint_slices
                .iter()
                .filter_map(|(_, _, endpoint_slice)| {
                    if let ObjectState::Active(endpoint_slice) = endpoint_slice {
                        let metadata = &endpoint_slice.metadata;
                        let labels = metadata.labels.as_ref()?;
                        labels
                            .get("kubernetes.io/service-name")
                            .map(|service_name| {
                                ObjectRef::new_builder()
                                    .of_kind::<Service>()
                                    .namespace(endpoint_slice.metadata.namespace.clone())
                                    .name(service_name)
                                    .build()
                                    .expect("Failed to build ObjectRef for Service")
                            })
                            .map(|service_ref| (service_ref, endpoint_slice.clone()))
                    } else {
                        None
                    }
                })
                .filter(|(object_ref, _)| current_http_route_backends.contains_key(object_ref))
                .map(|(object_ref, endpoint_slice)| {
                    (
                        object_ref.clone(),
                        extract_backend(&object_ref, endpoint_slice.as_ref()),
                    )
                })
                .collect();

            tx.replace(endpoint_slices_by_service);

            select_continue!(http_route_backends.changed(), endpoint_slices.changed());
        }
    });

    rx
}

fn extract_backend(object_ref: &ObjectRef, endpoint_slice: &EndpointSlice) -> Backend {
    let endpoints = endpoint_slice
        .endpoints
        .iter()
        .filter(
            |endpoint| match endpoint.conditions.clone().and_then(|c| c.ready) {
                Some(true) => true,
                _ => {
                    debug!(
                        "Skipping endpoint in EndpointSlice {:?}: not ready",
                        endpoint_slice.metadata.name
                    );
                    false
                }
            },
        )
        .map(|endpoint| {
            let zone_ref = Zone::new_builder()
                .zone(endpoint.zone.clone())
                .node(endpoint.node_name.clone())
                .build()
                .expect("Failed to build Zone");

            let addresses: Vec<_> = endpoint
                .addresses
                .iter()
                .flat_map(|a| match endpoint_slice.address_type.as_str() {
                    "IPv4" => a.parse::<Ipv4Addr>().ok().map(IpAddr::from),
                    "IPv6" => a.parse::<Ipv6Addr>().ok().map(IpAddr::from),
                    _ => None,
                })
                .collect();

            Endpoints::new_builder()
                .zone_ref(zone_ref)
                .addresses(addresses)
                .build()
                .expect("Failed to build Endpoints")
        })
        .collect();

    Backend::new_builder()
        .object_ref(object_ref.clone())
        .endpoints(endpoints)
        .build()
        .expect("Failed to build Backend")
}
