use anyhow::Result;
use clap::Parser;
use getset::{CloneGetters, CopyGetters, Getters};
use kubera_core::net::Port;
use std::path::PathBuf;

#[derive(Parser, Debug, Getters, Clone, CopyGetters, CloneGetters)]
#[command(name = "kubera-gateway")]
#[command(about = "The Kubera Gateway", long_about = None)]
pub struct Cli {
    #[getset(get_copy = "pub")]
    #[arg(default_value ="80",
          env = "PORT",
          long = "port",
          value_parser = parse_port,
    )]
    port: Port,

    #[getset(get_clone = "pub")]
    #[arg(
        default_value = "/etc/kubera/config.yaml",
        env = "KUBERA_GATEWAY_CONFIG_FILE_PATH",
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

    #[getset(get = "pub")]
    #[arg(env = "GATEWAY_NAME", long = "gateway-name")]
    gateway_name: String,
}

fn parse_port(arg: &str) -> Result<Port> {
    let port: u16 = arg.parse()?;
    Ok(Port::new(port))
}
