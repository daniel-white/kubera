use crate::controllers::transformers::http_routes::HttpRouteBackend;
use crate::kubernetes::objects::{ObjectRef, Objects, TopologyLocation};
use derive_builder::Builder;
use getset::Getters;
use k8s_openapi::api::core::v1::Service;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kubera_core::continue_on;
use kubera_core::sync::signal::{Receiver, channel};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use tokio::task::JoinSet;
use tracing::debug;

#[derive(Debug, Builder, Getters, Clone, Hash, PartialEq, Eq)]
pub struct Endpoints {
    #[getset(get = "pub")]
    location: TopologyLocation,

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
    weight: Option<i32>,

    #[getset(get = "pub")]
    port: Option<i32>,

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
    http_route_backends: &Receiver<HashMap<ObjectRef, HttpRouteBackend>>,
    endpoint_slices: &Receiver<Objects<EndpointSlice>>,
) -> Receiver<HashMap<ObjectRef, Backend>> {
    let (tx, rx) = channel(HashMap::new());

    let http_route_backends = http_route_backends.clone();
    let endpoint_slices = endpoint_slices.clone();

    join_set.spawn(async move {
        loop {
            let current_endpoint_slices = endpoint_slices.current();
            let current_http_route_backends = http_route_backends.current();

            let endpoint_slices_by_service: HashMap<_, _> = current_endpoint_slices
                .iter()
                .filter_map(|(_, _, endpoint_slice)| {
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
                        .map(|service_ref| (service_ref, endpoint_slice))
                })
                .filter_map(|(service_ref, endpoint_slice)| {
                    current_http_route_backends.get(&service_ref).map(|h| {
                        (
                            service_ref.clone(),
                            extract_backend(&service_ref, h, endpoint_slice.as_ref()),
                        )
                    })
                })
                .collect();

            tx.replace(endpoint_slices_by_service);

            continue_on!(http_route_backends.changed(), endpoint_slices.changed());
        }
    });

    rx
}

fn extract_backend(
    object_ref: &ObjectRef,
    http_route_backend: &HttpRouteBackend,
    endpoint_slice: &EndpointSlice,
) -> Backend {
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
            let location = TopologyLocation::new_builder()
                .zone(endpoint.zone.clone())
                .node(endpoint.node_name.clone())
                .build()
                .expect("Failed to build TopologyLocation");

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
                .location(location)
                .addresses(addresses)
                .build()
                .expect("Failed to build Endpoints")
        })
        .collect();

    Backend::new_builder()
        .object_ref(object_ref.clone())
        .endpoints(endpoints)
        .port(*http_route_backend.port())
        .weight(*http_route_backend.weight())
        .build()
        .expect("Failed to build Backend")
}
