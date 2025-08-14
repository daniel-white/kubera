use opentelemetry::global::meter;
use opentelemetry::metrics::Meter;
use std::sync::LazyLock;

pub(crate) static METER: LazyLock<Meter> = LazyLock::new(|| meter("vg-gateway"));
