mod context;

use async_trait::async_trait;
use context::Context;
use http::request::Parts;
use kubera_core::sync::signal::Receiver;
use pingora::prelude::*;
use std::ops::DerefMut;
use tracing::log::warn;
use tracing::{Instrument, info};

pub struct Gateway {}

impl Gateway {}

#[async_trait]
impl ProxyHttp for Gateway {
    type CTX = ();

    fn new_ctx(&self) -> Self::CTX {
        ()
    }

    async fn upstream_peer(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let req: &Parts = session.req_header();

        info!("Received request: {:?}", req);

        //  warn!("Query params: {:?}", x);

        // for matcher in self.matchers.current().matchers().iter() {
        //     if matcher.matches(req) {
        //         info!("Matched route: {:?}", matcher);
        //     }
        // }

        Err(Error::explain(HTTPStatus(503), "No matching route found"))
    }
}
