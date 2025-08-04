/// Test helper functions and utilities
pub struct TestHelper;

impl TestHelper {
    /// Initialize test environment with proper logging and configuration
    pub fn init() {
        let _ = env_logger::builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Debug)
            .try_init();
    }

    /// Create a test runtime for async tests
    pub fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create test runtime")
    }

    /// Generate random test data
    pub fn random_port() -> u16 {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen_range(1024..65535)
    }

    /// Create test configuration directory
    pub fn create_test_config_dir() -> tempfile::TempDir {
        tempfile::TempDir::new().expect("Failed to create test directory")
    }
}

/// Assertion helpers for testing
#[macro_export]
macro_rules! assert_eventually {
    ($condition:expr, $timeout:expr) => {{
        use std::time::{Duration, Instant};
        let start = Instant::now();
        let timeout_duration = Duration::from_millis($timeout);

        loop {
            if $condition {
                break;
            }

            if start.elapsed() > timeout_duration {
                panic!("Condition was not met within {}ms", $timeout);
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }};
}

/// Property-based test generators
pub mod generators {
    use proptest::prelude::*;

    pub fn valid_hostname() -> impl Strategy<Value = String> {
        prop::string::string_regex(
            r"[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(\.[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)*",
        )
        .expect("Invalid regex")
        .prop_filter("hostname too long", |s| s.len() <= 253)
    }

    pub fn valid_port() -> impl Strategy<Value = u16> {
        1u16..=65535
    }

    pub fn http_method() -> impl Strategy<Value = &'static str> {
        prop::sample::select(&["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"])
    }
}
