use crate::proxy::router::topology::{TopologyLocation, TopologyLocationMatch};
use getset::Getters;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::hash::{DefaultHasher, Hasher};
use std::net::{IpAddr, SocketAddr};
use enumflags2::BitFlags;

#[derive(Debug, Getters, Clone, PartialEq, Eq)]
pub struct EndpointsResolver {
    node_local: Vec<SocketAddr>,
    zone_local: Vec<SocketAddr>,
    fallback: Vec<SocketAddr>,
}

impl EndpointsResolver {
    pub fn builder(location: TopologyLocation) -> EndpointsResolverBuilder {
        EndpointsResolverBuilder::new(location)
    }

    pub fn resolve(&self, client_addr: Option<IpAddr>) -> impl Iterator<Item = SocketAddr> {
        let mut node_local = self.node_local.clone();
        let mut zone_local = self.zone_local.clone();
        let mut fallback = self.fallback.clone();

        let mut rng = match client_addr {
            Some(addr) => {
                let mut hasher = DefaultHasher::new();
                match addr {
                    IpAddr::V4(addr) => hasher.write(addr.octets().as_slice()),
                    IpAddr::V6(addr) => hasher.write(addr.octets().as_slice()),
                };

                let seed = hasher.finish();
                ChaCha8Rng::seed_from_u64(seed)
            }
            None => ChaCha8Rng::from_os_rng(),
        };
        node_local.shuffle(&mut rng);
        zone_local.shuffle(&mut rng);
        fallback.shuffle(&mut rng);

        node_local.into_iter().chain(zone_local).chain(fallback).fuse()
    }
}

#[derive(Debug)]
pub struct EndpointsResolverBuilder {
    current_location: TopologyLocation,
    node_local: Vec<SocketAddr>,
    zone_local: Vec<SocketAddr>,
    fallback: Vec<SocketAddr>,
}

impl EndpointsResolverBuilder {
    fn new(location: TopologyLocation) -> Self {
        Self {
            current_location: location,
            node_local: Vec::new(),
            zone_local: Vec::new(),
            fallback: Vec::new(),
        }
    }

    pub fn build(self) -> EndpointsResolver {
        EndpointsResolver {
            node_local: self.node_local,
            zone_local: self.zone_local,
            fallback: self.fallback,
        }
    }

pub fn insert(&mut self, addr: SocketAddr, location_match: BitFlags<TopologyLocationMatch>) -> &mut Self {
        if location_match.contains(TopologyLocationMatch::Node) {
            self.node_local.push(addr);
        } else if location_match.contains(TopologyLocationMatch::Zone) {
            self.zone_local.push(addr);
        } else {
            self.fallback.push(addr);
        }

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::str::FromStr;

    #[test]
    fn test_topology_addrs_sequence_with_location_builder() {
        let node_ip1: SocketAddr = "192.168.1.1:8080".parse().unwrap();
        let node_ip2: SocketAddr = "192.168.1.2:8080".parse().unwrap();
        let zone_ip1: SocketAddr = "192.168.2.1:8080".parse().unwrap();
        let zone_ip2: SocketAddr = "192.168.2.2:8080".parse().unwrap();
        let fallback_ip1: SocketAddr = "192.168.3.1:8080".parse().unwrap();
        let fallback_ip2: SocketAddr = "192.168.3.2:8080".parse().unwrap();

        let location = TopologyLocation::builder()
            .node(Some("node1".to_string()))
            .zone(Some("zone1".to_string()))
            .build();
        let mut addr_builder = EndpointsResolver::builder(location.clone());

        // Insert node-local addresses
        addr_builder.insert(node_ip1, BitFlags::from(TopologyLocationMatch::Node));
        addr_builder.insert(node_ip2, BitFlags::from(TopologyLocationMatch::Node));

        // Insert zone-local addresses
        addr_builder.insert(zone_ip1, BitFlags::from(TopologyLocationMatch::Zone));
        addr_builder.insert(zone_ip2, BitFlags::from(TopologyLocationMatch::Zone));

        // Insert fallback addresses
        addr_builder.insert(fallback_ip1, BitFlags::empty());
        addr_builder.insert(fallback_ip2, BitFlags::empty());

        let topology_addrs = addr_builder.build();

        let sequence: Vec<SocketAddr> = topology_addrs
            .resolve(Some(IpAddr::from_str("127.0.0.1").unwrap()))
            .collect();

        // Assert that node addresses come first
        assert!(sequence.starts_with(&[node_ip1, node_ip2]));

        // Assert that zone addresses come next
        assert!(sequence.contains(&zone_ip1));
        assert!(sequence.contains(&zone_ip2));

        // Assert that fallback addresses come last
        assert!(sequence.ends_with(&[fallback_ip1, fallback_ip2]));
    }
}
