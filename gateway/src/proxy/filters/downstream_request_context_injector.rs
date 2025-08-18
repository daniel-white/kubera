use http::HeaderMap;
use opentelemetry::Context;
use opentelemetry::propagation::TextMapPropagator;
use opentelemetry_http::HeaderInjector;

/// Filter that applies an OpenTelemetry propagator to upstream requests
pub struct UpstreamRequestContextInjectorFilter<'a>(&'a dyn TextMapPropagator);

impl<'a> UpstreamRequestContextInjectorFilter<'a> {
    pub fn new(propagator: &'a dyn TextMapPropagator) -> Self {
        Self(propagator)
    }

    pub fn apply_to_headers(&self, headers: &mut HeaderMap) {
        let propagator = &*self.0;

        for field in propagator.fields() {
            headers.remove(field);
        }

        let mut injector = HeaderInjector(headers);
        propagator.inject_context(&Context::current(), &mut injector);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;
    use opentelemetry::Context;
    use opentelemetry::propagation::text_map_propagator::FieldIter;
    use opentelemetry::propagation::{Extractor, Injector, TextMapPropagator};
    use std::sync::OnceLock;

    static TRACE_CONTEXT_HEADER_FIELDS: OnceLock<[String; 2]> = OnceLock::new();

    fn trace_context_header_fields() -> &'static [String; 2] {
        TRACE_CONTEXT_HEADER_FIELDS
            .get_or_init(|| ["traceparent".to_owned(), "tracestate".to_owned()])
    }

    #[derive(Debug)]
    struct DummyPropagator;
    impl TextMapPropagator for DummyPropagator {
        fn inject_context(&self, _cx: &Context, injector: &mut dyn Injector) {
            injector.set("traceparent", "dummy-parent".to_string());
            injector.set("tracestate", "dummy-state".to_string());
        }

        fn extract(&self, _extractor: &dyn Extractor) -> Context {
            Context::new()
        }

        fn extract_with_context(&self, _cx: &Context, _extractor: &dyn Extractor) -> Context {
            Context::new()
        }

        fn fields(&self) -> FieldIter<'_> {
            FieldIter::new(trace_context_header_fields())
        }
    }

    #[test]
    fn test_removes_and_injects_trace_headers() {
        let propagator = DummyPropagator;
        let filter = UpstreamRequestContextInjectorFilter::new(&propagator);
        let mut headers = HeaderMap::new();
        headers.append("traceparent", HeaderValue::from_static("old-parent"));
        headers.append("tracestate", HeaderValue::from_static("old-state"));
        headers.append("other", HeaderValue::from_static("value"));

        filter.apply_to_headers(&mut headers);
        assert_eq!(
            headers.get("traceparent"),
            Some(&HeaderValue::from_static("dummy-parent"))
        );
        assert_eq!(
            headers.get("tracestate"),
            Some(&HeaderValue::from_static("dummy-state"))
        );
        assert_eq!(
            headers.get("other"),
            Some(&HeaderValue::from_static("value"))
        );
    }
}
