pub fn init_logging() {
    flexi_logger::Logger::try_with_env_or_str("info")
        .expect("Failed to initialize logger")
        .start()
        .expect("Failed to start logger");
}
