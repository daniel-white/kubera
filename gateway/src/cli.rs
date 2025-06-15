use anyhow::Result;
use clap::Parser;
use getset::{CopyGetters, Getters};
use kubera_core::net::Port;
use std::path::PathBuf;

#[derive(Parser, Debug, Getters, Clone, CopyGetters)]
#[command(name = "kubera-gateway")]
#[command(about = "The Kubera Gateway", long_about = None)]
pub struct Cli {
    #[getset(get_copy = "pub")]
    #[arg(default_value ="8080",
          env = "KUBERA_GATEWAY_PROXY_PORT",
          long = "proxy-port",
          value_parser = parse_port,
    )]
    proxy_port: Port,

    #[getset(get_copy = "pub")]
    #[arg(default_value ="8081",
          env = "KUBERA_GATEWAY_CONTROL_SERVICE_PORT",
          long = "control-service-port",
          value_parser = parse_port
    )]
    control_service_port: Port,

    #[getset(get = "pub")]
    #[arg(
        default_value = "gateway.yaml",
        env = "KUBERA_GATEWAY_CONFIG_FILE_PATH",
        long = "config-file-path"
    )]
    config_file_path: PathBuf,
}

fn parse_port(arg: &str) -> Result<Port> {
    let port: u16 = arg.parse()?;
    Ok(Port::new(port))
}
