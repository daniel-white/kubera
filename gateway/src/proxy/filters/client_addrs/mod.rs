use http::HeaderName;

pub mod controller;
mod handler;
mod extractors;

const VALE_GATEWAY_CLIENT_IP_HEADER: HeaderName = HeaderName::from_static("vale-gateway-client-ip");

pub use handler::ClientAddrFilterHandler;