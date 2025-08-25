use clap::Parser;
use getset::{CloneGetters, CopyGetters, Getters};
use std::path::PathBuf;

#[derive(Parser, Debug, Getters, Clone, CopyGetters, CloneGetters)]
#[command(name = "vale-gateway")]
#[command(about = "The Vale Gateway", long_about = None)]
pub struct Cli {
    #[getset(get_clone = "pub")]
    #[arg(
        default_value = "/etc/vale-gateway/config.yaml",
        env = "VALE_GATEWAY_CONFIG_FILE_PATH",
        long = "config-file-path"
    )]
    config_file_path: PathBuf,

    #[getset(get_clone = "pub")]
    #[arg(env = "NODE_NAME", long = "node-name")]
    node_name: Option<String>,

    #[getset(get_clone = "pub")]
    #[arg(env = "ZONE_NAME", long = "zone-name")]
    zone_name: Option<String>,

    #[getset(get_clone = "pub")]
    #[arg(env = "POD_NAMESPACE", long = "namespace")]
    pod_namespace: String,

    #[getset(get_clone = "pub")]
    #[arg(env = "POD_NAME", long = "pod-name")]
    pod_name: String,

    #[getset(get_clone = "pub")]
    #[arg(env = "GATEWAY_NAME", long = "gateway-name")]
    gateway_name: String,

    #[getset(get_clone = "pub")]
    #[arg(env = "VALE_GATEWAY_LISTENERS", long = "listeners")]
    vale_gateway_listeners: Option<String>,
}
