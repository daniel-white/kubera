use crate::proxy::router::topology::TopologyLocationMatch;
use enumflags2::BitFlags;
use getset::Getters;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;
use std::hash::{DefaultHasher, Hasher};
use std::net::{IpAddr, SocketAddr};
use tracing::{debug, warn};

#[derive(Debug, Getters, Clone, PartialEq, Eq)]
pub struct EndpointsResolver {
    endpoints: Vec<SocketAddr>,
    attempt: usize,
}

impl EndpointsResolver {
    pub fn builder(client_addr: Option<IpAddr>) -> EndpointsResolverBuilder {
        EndpointsResolverBuilder::new(client_addr)
    }

    pub fn next(&mut self) -> Option<SocketAddr> {
        if self.endpoints.is_empty() || self.attempt >= 5 {
            // Arbitrary limit to avoid infinite loop; TODO: Make configurable
            warn!("No endpoints available or too many attempts made");
            return None;
        }

        let addr = self.endpoints[self.attempt % self.endpoints.len()];
        self.attempt += 1;
        debug!("On attempt {} using endpoint: {}", self.attempt, addr);
        Some(addr)
    }
}

#[derive(Debug)]
pub struct EndpointsResolverBuilder {
    client_addr: Option<IpAddr>,
    unique_id: Option<String>,
    node_local: Vec<SocketAddr>,
    zone_local: Vec<SocketAddr>,
    fallback: Vec<SocketAddr>,
}

impl EndpointsResolverBuilder {
    fn new(client_addr: Option<IpAddr>) -> Self {
        Self {
            client_addr,
            unique_id: None,
            node_local: Vec::new(),
            zone_local: Vec::new(),
            fallback: Vec::new(),
        }
    }

    pub fn unique_id<S: AsRef<str>>(&mut self, unique_id: S) -> &mut Self {
        self.unique_id = Some(unique_id.as_ref().to_string());
        self
    }

    pub fn insert(
        &mut self,
        addr: SocketAddr,
        location_match: BitFlags<TopologyLocationMatch>,
    ) -> &mut Self {
        if location_match.contains(TopologyLocationMatch::Node) {
            self.node_local.push(addr);
        } else if location_match.contains(TopologyLocationMatch::Zone) {
            self.zone_local.push(addr);
        } else {
            self.fallback.push(addr);
        }

        self
    }

    pub fn build(self) -> EndpointsResolver {
        let mut node_local = self.node_local.clone();
        let mut zone_local = self.zone_local.clone();
        let mut fallback = self.fallback.clone();

        let mut rng = match self.client_addr {
            Some(addr) => {
                let mut hasher = DefaultHasher::new();
                match addr {
                    IpAddr::V4(addr) => hasher.write(addr.octets().as_slice()),
                    IpAddr::V6(addr) => hasher.write(addr.octets().as_slice()),
                };

                if let Some(unique_id) = &self.unique_id {
                    hasher.write(unique_id.as_bytes());
                }

                let seed = hasher.finish();
                ChaCha8Rng::seed_from_u64(seed)
            }
            None => ChaCha8Rng::from_os_rng(),
        };
        node_local.shuffle(&mut rng);
        zone_local.shuffle(&mut rng);
        fallback.shuffle(&mut rng);

        EndpointsResolver {
            endpoints: node_local
                .into_iter()
                .chain(zone_local)
                .chain(fallback)
                .collect(),
            attempt: 0,
        }
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

        let mut resolver_builder =
            EndpointsResolver::builder(Some(IpAddr::from_str("127.0.0.1").unwrap()));

        // Insert node-local addresses
        resolver_builder.insert(node_ip1, BitFlags::from(TopologyLocationMatch::Node));
        resolver_builder.insert(node_ip2, BitFlags::from(TopologyLocationMatch::Node));

        // Insert zone-local addresses
        resolver_builder.insert(zone_ip1, BitFlags::from(TopologyLocationMatch::Zone));
        resolver_builder.insert(zone_ip2, BitFlags::from(TopologyLocationMatch::Zone));

        // Insert fallback addresses
        resolver_builder.insert(fallback_ip1, BitFlags::empty());
        resolver_builder.insert(fallback_ip2, BitFlags::empty());

        let resolver = resolver_builder.build();
        let endpoints = resolver.endpoints;

        // Assert that node addresses come first
        assert!(endpoints.starts_with(&[node_ip1, node_ip2]));

        // Assert that zone addresses come next
        assert!(endpoints.contains(&zone_ip1));
        assert!(endpoints.contains(&zone_ip2));

        // Assert that fallback addresses come last
        assert!(endpoints.ends_with(&[fallback_ip1, fallback_ip2]));
    }
}
