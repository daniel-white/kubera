use opentelemetry::global::{BoxedTracer, meter, tracer};
use opentelemetry::metrics::Meter;
use std::sync::LazyLock;

pub(crate) static METER: LazyLock<Meter> = LazyLock::new(|| meter("vg-gateway"));

pub(crate) static TRACER: LazyLock<BoxedTracer> = LazyLock::new(|| tracer("vg-gateway"));
