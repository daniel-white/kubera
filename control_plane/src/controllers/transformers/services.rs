use crate::controllers::transformers::http_routes::HttpRouteBackend;
use crate::kubernetes::objects::{ObjectRef, Objects, TopologyLocation};
use getset::{CopyGetters, Getters};
use k8s_openapi::api::core::v1::Service;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use tracing::debug;
use typed_builder::TypedBuilder;
use vg_core::continue_on;
use vg_core::net::Port;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_macros::await_ready;

#[derive(Debug, TypedBuilder, Getters, Clone, Hash, PartialEq, Eq)]
pub struct Endpoints {
    #[getset(get = "pub")]
    location: TopologyLocation,

    #[getset(get = "pub")]
    #[builder(setter(into))]
    addresses: Vec<IpAddr>,
}

#[derive(Debug, TypedBuilder, Getters, CopyGetters, Clone, Hash, PartialEq, Eq)]
pub struct Backend {
    #[getset(get_copy = "pub")]
    #[builder(default)]
    weight: Option<i32>,

    #[getset(get_copy = "pub")]
    #[builder(setter(into))]
    port: Option<Port>,

    #[getset(get = "pub")]
    object_ref: ObjectRef,

    #[getset(get = "pub")]
    endpoints: Vec<Endpoints>,
}

pub fn collect_service_backends(
    task_builder: &TaskBuilder,
    http_route_backends_rx: &Receiver<HashMap<ObjectRef, HttpRouteBackend>>,
    endpoint_slices_rx: &Receiver<Objects<EndpointSlice>>,
) -> Receiver<HashMap<ObjectRef, Backend>> {
    let (tx, rx) = signal("collected_service_backends");
    let http_route_backends_rx = http_route_backends_rx.clone();
    let endpoint_slices_rx = endpoint_slices_rx.clone();

    task_builder
        .new_task(stringify!(collect_service_backends))
        .spawn(async move {
            loop {
                await_ready!(http_route_backends_rx, endpoint_slices_rx)
                    .and_then(async |http_route_backends, endpoint_slices| {
                        let endpoint_slices_by_service: HashMap<_, _> = endpoint_slices
                            .iter()
                            .filter_map(|(_, _, endpoint_slice)| {
                                let metadata = &endpoint_slice.metadata;
                                let labels = metadata.labels.as_ref()?;
                                labels
                                    .get("kubernetes.io/service-name")
                                    .map(|service_name| {
                                        ObjectRef::of_kind::<Service>()
                                            .namespace(endpoint_slice.metadata.namespace.clone())
                                            .name(service_name)
                                            .build()
                                    })
                                    .map(|service_ref| (service_ref, endpoint_slice))
                            })
                            .filter_map(|(service_ref, endpoint_slice)| {
                                http_route_backends.get(&service_ref).map(|h| {
                                    (
                                        service_ref.clone(),
                                        extract_backend(&service_ref, h, endpoint_slice.as_ref()),
                                    )
                                })
                            })
                            .collect();

                        tx.set(endpoint_slices_by_service).await;
                    })
                    .run()
                    .await;

                continue_on!(
                    http_route_backends_rx.changed(),
                    endpoint_slices_rx.changed()
                );
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
        .filter(|endpoint| {
            if let Some(true) = endpoint.conditions.clone().and_then(|c| c.ready) {
                true
            } else {
                debug!(
                    "Skipping endpoint in EndpointSlice {:?}: not ready",
                    endpoint_slice.metadata.name
                );
                false
            }
        })
        .map(|endpoint| {
            let location = TopologyLocation::builder()
                .zone(endpoint.zone.clone().unwrap_or_default())
                .node(endpoint.node_name.clone().unwrap_or_default())
                .build();

            let addresses: Vec<_> = endpoint
                .addresses
                .iter()
                .filter_map(|a| match endpoint_slice.address_type.as_str() {
                    "IPv4" => a.parse::<Ipv4Addr>().ok().map(IpAddr::from),
                    "IPv6" => a.parse::<Ipv6Addr>().ok().map(IpAddr::from),
                    _ => None,
                })
                .collect();

            Endpoints::builder()
                .location(location)
                .addresses(addresses)
                .build()
        })
        .collect();

    Backend::builder()
        .object_ref(object_ref.clone())
        .endpoints(endpoints)
        .port(http_route_backend.port())
        .weight(http_route_backend.weight())
        .build()
}
