use async_trait::async_trait;
use pingora::lb::discovery::ServiceDiscovery;
use pingora::lb::Backend;
use std::collections::{BTreeSet, HashMap};

struct BackendServicesDiscovery;

#[async_trait]
impl ServiceDiscovery for BackendServicesDiscovery {
    async fn discover(&self) -> pingora::Result<(BTreeSet<Backend>, HashMap<u64, bool>)> {
        todo!()
    }
}
