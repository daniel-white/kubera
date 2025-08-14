use crate::instrumentation::METER;
use opentelemetry::metrics::Counter;
use opentelemetry::KeyValue;
use std::sync::LazyLock;
use tracing::trace;

static SET_APPLIED: LazyLock<Counter<u64>> = LazyLock::new(|| {
    METER
        .u64_counter("signal_set_applied")
        .with_description("Number of times a signal value was set")
        .build()
});

#[track_caller]
#[inline]
pub fn record_set_applied(name: &'static str) {
    trace!("Set value in signal");
    SET_APPLIED.add(1, &[KeyValue::new("signal", name)]);
}

#[track_caller]
#[inline]
pub fn record_set_skipped(name: &'static str) {
    trace!("Skipped setting value in signal");
}
