use crate::instrumentation::get_meter;
use getset::Getters;
use http::header::{HOST, USER_AGENT};
use http::uri::Authority;
use http::{request, response, HeaderMap, StatusCode};
use opentelemetry::global::get_text_map_propagator;
use opentelemetry::metrics::{Histogram, UpDownCounter};
use opentelemetry::KeyValue;
use opentelemetry_http::{HeaderExtractor, HeaderInjector};
use opentelemetry_semantic_conventions::metric::HTTP_SERVER_REQUEST_DURATION;
use opentelemetry_semantic_conventions::trace::*;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr};
use std::sync::LazyLock;
use std::time::Instant;
use tracing::span::EnteredSpan;
use tracing::{info_span, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

static DURATION: LazyLock<Histogram<f64>> = LazyLock::new(|| {
    get_meter()
        .f64_histogram(HTTP_SERVER_REQUEST_DURATION)
        .with_description("Duration of HTTP server requests.")
        .with_unit("s")
        .with_boundaries(vec![
            0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0,
        ])
        .build()
});

static ACTIVE_REQUESTS: LazyLock<UpDownCounter<i64>> = LazyLock::new(|| {
    get_meter()
        .i64_up_down_counter("http.server.active_requests")
        .with_description("Number of active HTTP server requests.")
        .build()
});

#[derive(Debug, Getters)]
pub struct RequestInstrumentation {
    start_time: Instant,

    duration_attributes: RefCell<Vec<KeyValue>>,
    active_requests_attributes: RefCell<Vec<KeyValue>>,

    #[getset(get = "pub")]
    request_span: Span,

    upstream_request_spans: RefCell<VecDeque<EnteredSpan>>,
}

impl RequestInstrumentation {
    #[track_caller]
    pub fn new() -> Self {
        let start_time = Instant::now();
        let request_span = info_span!("request", otel.kind = "server");
        Self {
            start_time,
            duration_attributes: RefCell::default(),
            active_requests_attributes: RefCell::default(),
            request_span,
            upstream_request_spans: RefCell::default(),
        }
    }

    #[track_caller]
    pub fn record_request(&self, req: &request::Parts) {
        let mut span_attrs: Vec<KeyValue> = Vec::with_capacity(16);
        let mut active_requests_attrs: Vec<KeyValue> = Vec::with_capacity(16);
        let mut duration_attrs: Vec<KeyValue> = Vec::with_capacity(16);
        let context = get_text_map_propagator(|p| {
            let extractor = HeaderExtractor(&req.headers);
            p.extract(&extractor)
        });
        self.request_span.set_parent(context);

        let uri = &req.uri;

        {
            let http_method = req.method.as_str().to_ascii_uppercase();
            // special case for renaming the trace name
            self.request_span.record("otel.name", http_method.clone());
            let attr = KeyValue::new(HTTP_REQUEST_METHOD, http_method);
            span_attrs.push(attr.clone());
            active_requests_attrs.push(attr.clone());
            duration_attrs.push(attr);
        }

        span_attrs.push(KeyValue::new(URL_PATH, uri.path().to_string()));

        if let Some(scheme) = uri.scheme() {
            let scheme = scheme.to_string().to_ascii_lowercase();
            let scheme = KeyValue::new(URL_SCHEME, scheme.clone());
            span_attrs.push(scheme.clone());
            active_requests_attrs.push(scheme.clone());
            duration_attrs.push(scheme);
        }

        if let Some(query) = uri.query() {
            span_attrs.push(KeyValue::new(URL_QUERY, query.to_string()));
        }

        let authority = req
            .headers
            .get(HOST)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.parse::<Authority>().ok());
        if let Some(host) = authority.as_ref().map(|a| a.host().to_string()) {
            let server_address = KeyValue::new(SERVER_ADDRESS, host.clone());
            span_attrs.push(server_address.clone());
            active_requests_attrs.push(server_address.clone());
            duration_attrs.push(server_address.clone());
        }
        if let Some(port) = authority.and_then(|a| a.port_u16()) {
            let server_port = KeyValue::new(SERVER_PORT, port as i64);
            span_attrs.push(server_port.clone());
            active_requests_attrs.push(server_port.clone());
            duration_attrs.push(server_port);
        }

        let user_agent = req.headers.get(USER_AGENT).and_then(|h| h.to_str().ok());
        if let Some(user_agent) = user_agent {
            span_attrs.push(KeyValue::new(USER_AGENT_ORIGINAL, user_agent.to_string()));
        }
        
        
        for kv in span_attrs.into_iter() {
            self.request_span.set_attribute(kv.key, kv.value);
        }
        
        self.duration_attributes.borrow_mut().extend(duration_attrs);

        ACTIVE_REQUESTS.add(1, &active_requests_attrs);
        self.active_requests_attributes.borrow_mut().extend(active_requests_attrs);
    }

    #[track_caller]
    pub fn record_client_addr(&self, addr: Option<IpAddr>) {
        if let Some(addr) = addr {
            self.request_span
                .set_attribute(CLIENT_ADDRESS, addr.to_string());
        } else {
            self.request_span
                .set_attribute(CLIENT_ADDRESS, "".to_string());
        }
    }

    #[track_caller]
    pub fn record_upstream_peer(&self, addr: SocketAddr) {
        let span = Span::current();
        span.set_attribute(NETWORK_PEER_ADDRESS, addr.ip().to_string());
        span.set_attribute(NETWORK_PEER_PORT, addr.port() as i64);
    }

    #[track_caller]
    pub fn begin_upstream_call(&self, upstream_req_headers: &mut HeaderMap) {
        let span = info_span!("upstream_request", otel.kind = "client",);
        span.set_parent(self.request_span.context());

        get_text_map_propagator(|p| {
            let mut injector = HeaderInjector(upstream_req_headers);
            p.inject(&mut injector);
        });

        self.upstream_request_spans
            .borrow_mut()
            .push_back(span.entered());
    }

    #[track_caller]
    pub fn end_upstream_call(&self, upstream_res: &response::Parts) {
        if let Some(span) = self.upstream_request_spans.borrow_mut().pop_back() {
            self.request_span.set_attribute(
                HTTP_RESPONSE_STATUS_CODE,
                upstream_res.status.as_u16() as i64,
            );
            span.exit();
        }
    }

    #[track_caller]
    pub fn record_status(&self, status: StatusCode) {
        let mut duration_attributes = self.duration_attributes.borrow_mut();
        let status = status.as_u16() as i64;
        self.request_span
            .set_attribute(HTTP_RESPONSE_STATUS_CODE, status);
        self.request_span.set_attribute("http.status_code", status);
        duration_attributes.push(KeyValue::new(HTTP_RESPONSE_STATUS_CODE, status));
    }
}

impl Drop for RequestInstrumentation {
    fn drop(&mut self) {
        let duration = self.start_time.elapsed();
        let duration_attributes = self.duration_attributes.borrow();
        DURATION.record(duration.as_secs_f64(), &duration_attributes);
        let active_requests_attributes = self.active_requests_attributes.borrow();
        ACTIVE_REQUESTS.add(-1, &active_requests_attributes);
    }
}
