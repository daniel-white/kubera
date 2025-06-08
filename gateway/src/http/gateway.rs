use crate::config::matchers_controller::Matchers;
use crate::http::extensions::QueryParamsExtension;
use crate::http::route_matcher::Matcher;
use async_trait::async_trait;
use http::request::Parts;
use kubera_core::sync::signal::Receiver;
use pingora::prelude::*;
use std::ops::DerefMut;
use tracing::log::warn;
use tracing::{info, Instrument};

pub struct Gateway {
    matchers: Receiver<Matchers>,
}

impl Gateway {
    pub fn new(matchers: Receiver<Matchers>) -> Self {
        Gateway { matchers }
    }
}

#[async_trait]
impl ProxyHttp for Gateway {
    type CTX = ();
    fn new_ctx(&self) -> Self::CTX {}

    async fn early_request_filter(&self, session: &mut Session, _ctx: &mut Self::CTX) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        let req = session.req_header_mut();
        let query_params_extension = QueryParamsExtension::from_uri(&req.uri);
        req.as_owned_parts()
        req.extensions.insert(query_params_extension);
        info!("early filter");
        Ok(())
    }

    async fn upstream_peer(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let req: &Parts = session.req_header();

        info!("Received request: {:?}", req);

        let x = req.extensions.get::<QueryParamsExtension>();
        warn!("Query params: {:?}", x);

        for matcher in self.matchers.current().matchers().iter() {
            if matcher.matches(req) {
                info!("Matched route: {:?}", matcher);
            }
        }

        Err(Error::explain(HTTPStatus(503), "No matching route found"))
    }
}
