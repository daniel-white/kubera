use opentelemetry::global::{meter, tracer, BoxedTracer};
use opentelemetry::metrics::Meter;
use std::sync::LazyLock;

static METER: LazyLock<Meter> = LazyLock::new(|| meter("vg-gateway"));

pub fn get_meter() -> &'static Meter {
    &METER
}

static TRACER: LazyLock<BoxedTracer> = LazyLock::new(|| tracer("vg-gateway"));

pub fn get_tracer() -> &'static BoxedTracer {
    &TRACER
}
