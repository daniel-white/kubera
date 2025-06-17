use anyhow::Result;
use clap::{Parser, Subcommand};
use getset::Getters;
use kubera_core::net::Port;

#[derive(Parser, Getters)]
#[command(name = "kubera-controlplane")]
#[command(about = "A Kubernetes control plane for Kubera", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    #[getset(get = "pub")]
    command: Option<Commands>,

    #[getset(get_copy = "pub")]
    #[arg(default_value ="8081",
          env = "KUBERA_CONTROL_SERVICE_PORT",
          long = "control-service-port",
          value_parser = parse_port,
    )]
    control_service_port: Port,
}

#[derive(Subcommand)]
pub enum Commands {
    Run,
    WriteCrds {
        #[arg(short, long)]
        output_path: Option<String>,
    },
}

fn parse_port(arg: &str) -> Result<Port> {
    let port: u16 = arg.parse()?;
    Ok(Port::new(port))
}
