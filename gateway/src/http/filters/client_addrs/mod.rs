use http::HeaderName;

mod controllers;
mod extractors;
mod handler;

const VALE_GATEWAY_CLIENT_IP_HEADER: HeaderName = HeaderName::from_static("vale-gateway-client-ip");

pub use controllers::*;
pub use handler::*;
