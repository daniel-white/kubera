use http::HeaderMap;
use opentelemetry::Context;
use opentelemetry::propagation::TextMapPropagator;
use opentelemetry_http::HeaderExtractor;

/// Filter that extracts from requests
pub struct RequestContextExtractorFilter<'a>(&'a dyn TextMapPropagator);

impl<'a> RequestContextExtractorFilter<'a> {
    pub fn new(propagator: &'a dyn TextMapPropagator) -> Self {
        Self(propagator)
    }

    pub fn extract_from_headers(&self, headers: &HeaderMap) -> Context {
        let extractor = HeaderExtractor(headers);
        self.0.extract(&extractor)
    }
}
