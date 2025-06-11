mod http_routes;
mod services;

pub use http_routes::collect_http_route_service_backends;
pub use services::collect_service_endpoint_ips;
