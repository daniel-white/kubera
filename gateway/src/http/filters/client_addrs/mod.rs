use http::HeaderName;

mod controller;
mod extractors;
mod handler;

const VALE_GATEWAY_CLIENT_IP_HEADER: HeaderName = HeaderName::from_static("vale-gateway-client-ip");

pub use controller::client_addr_filter_handler;
pub use handler::*;
