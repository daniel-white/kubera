use std::sync::Once;

static INIT: Once = Once::new();

pub fn init_instrumentation() {
    INIT.call_once(|| {
        #[cfg(debug_assertions)]
        unsafe {
            backtrace_on_stack_overflow::enable();
        };

        // Check if we should use console subscriber (for tokio-console debugging)
        // This is determined by the TOKIO_CONSOLE_BIND environment variable
        if std::env::var("TOKIO_CONSOLE_BIND").is_ok() {
            console_subscriber::init();
        } else {
            // Use tracing subscriber with stdout output for regular logging
            let subscriber = tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
                )
                .with_target(false)
                .with_thread_ids(true)
                .with_level(true)
                .finish();

            // Set the global subscriber
            #[allow(clippy::expect_used)]
            // Expect is used here because we are initializing the logger at startup
            // and failure to set up logging should cause the application to fail fast
            tracing::subscriber::set_global_default(subscriber)
                .expect("Failed to set tracing subscriber");
        }
    });
}
