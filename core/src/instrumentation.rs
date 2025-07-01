pub fn init_instrumentation() {
    #[cfg(debug_assertions)]
    unsafe {
        backtrace_on_stack_overflow::enable()
    };

    flexi_logger::Logger::try_with_env_or_str("info")
        .expect("Failed to initialize logger")
        .start()
        .expect("Failed to start logger");
}
