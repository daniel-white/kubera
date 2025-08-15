use crate::task::Builder as TaskBuilder;
use opentelemetry::Context;
use opentelemetry::global::{meter, set_meter_provider, set_tracer_provider};
use opentelemetry::metrics::Meter;
use opentelemetry::trace::{TraceContextExt, TracerProvider};
use opentelemetry_appender_log::OpenTelemetryLogBridge;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporterBuilder, MetricExporterBuilder, SpanExporterBuilder};
use opentelemetry_sdk::logs::LoggerProviderBuilder;
use opentelemetry_sdk::metrics::MeterProviderBuilder;
use opentelemetry_sdk::trace::TracerProviderBuilder;
use std::sync::{LazyLock, Once};
use tracing::info;
use tracing::log::set_boxed_logger;
use tracing::subscriber::set_global_default;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::fmt::layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Registry};

#[allow(clippy::expect_used)]
#[track_caller]
pub fn init_instrumentation(task_builder: &TaskBuilder, name: &'static str) {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        #[cfg(debug_assertions)]
        unsafe {
            backtrace_on_stack_overflow::enable();
        };

        let otlp_metrics_exporter = MetricExporterBuilder::new()
            .with_tonic()
            .build()
            .expect("Failed to create OTLP exporter");

        let meter_provider = MeterProviderBuilder::default()
            .with_periodic_exporter(otlp_metrics_exporter)
            .build();
        set_meter_provider(meter_provider.clone());

        let otlp_logs_exporter = LogExporterBuilder::new()
            .with_tonic()
            .build()
            .expect("Failed to create OTLP log exporter");

        let logs_provider = LoggerProviderBuilder::default()
            .with_batch_exporter(otlp_logs_exporter)
            .build();

        let logger = OpenTelemetryLogBridge::new(&logs_provider);
        set_boxed_logger(Box::new(logger)).expect("Failed to set logger");

        let otlp_span_exporter = SpanExporterBuilder::new()
            .with_tonic()
            .build()
            .expect("Failed to create OTLP trace exporter");

        let tracer_provider = TracerProviderBuilder::default()
            .with_batch_exporter(otlp_span_exporter)
            .build();

        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));

        let tracer = tracer_provider.tracer(name);
        let otel_trace_layer = OpenTelemetryLayer::new(tracer);
        let otel_log_layer = OpenTelemetryTracingBridge::new(&logs_provider);
        let stdout_layer = layer()
            .json()
            .with_level(true)
            .with_current_span(true)
            .with_target(true);
        let console_layer = console_subscriber::spawn();
        let registry = Registry::default()
            .with(env_filter)
            .with(console_layer)
            .with(otel_trace_layer)
            .with(otel_log_layer)
            .with(stdout_layer);

        set_global_default(registry).expect("Failed to set global tracing subscriber");
        set_tracer_provider(tracer_provider.clone());

        task_builder
            .new_task("shutdown_instrumentation")
            .spawn_on_shutdown(async move {
                info!("Shutting down instrumentation...");
                let _ = tracer_provider.shutdown();
                let _ = meter_provider.shutdown();
                let _ = logs_provider.shutdown();
            });
    });
}

pub(crate) static METER: LazyLock<Meter> = LazyLock::new(|| meter("vg-core"));

pub fn trace_id() -> Option<String> {
    let context = Context::current();
    if context.has_active_span() {
        context.span().span_context().trace_id().to_string().into()
    } else {
        None
    }
}
