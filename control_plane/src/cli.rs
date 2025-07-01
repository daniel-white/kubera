use clap::Parser;
use getset::{CopyGetters, Getters};

#[derive(Parser, Getters, CopyGetters)]
#[command(about = "A Kubernetes control plane for Kubera", long_about = None)]
pub struct Cli {
    #[getset(get_copy = "pub")]
    #[arg(default_value = "8080", env = "PORT", long = "port")]
    port: u16,

    #[getset(get = "pub")]
    #[arg(env = "POD_NAMESPACE", long = "namespace")]
    pod_namespace: String,

    #[getset(get = "pub")]
    #[arg(env = "POD_NAME", long = "pod-name")]
    pod_name: String,

    #[getset(get = "pub")]
    #[arg(env = "KUBERA_INSTANCE", long = "instance")]
    instance_name: String,
}
