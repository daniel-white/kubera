use getset::{CopyGetters, Getters};
use std::time::Duration;

#[derive(Debug, Getters, CopyGetters, PartialEq, Eq)]
pub struct Options {
    #[getset(get_copy = "pub")]
    lease_duration: Duration,

    #[getset(get_copy = "pub")]
    lease_check_interval: Duration,

    #[getset(get_copy = "pub")]
    auto_cycle_duration: Duration,

    #[getset(get_copy = "pub")]
    controller_requeue_duration: Duration,

    #[getset(get_copy = "pub")]
    controller_error_requeue_duration: Duration,
    
    #[getset(get_copy = "pub")]
    ipc_sse_keep_alive_interval: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            lease_duration: Duration::from_secs(20),
            lease_check_interval: Duration::from_secs(5),
            auto_cycle_duration: Duration::from_secs(15),
            controller_requeue_duration: Duration::from_secs(60),
            controller_error_requeue_duration: Duration::from_secs(5),
            ipc_sse_keep_alive_interval: Duration::from_secs(15),
        }
    }
}
