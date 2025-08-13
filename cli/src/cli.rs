use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "vgctl",
    about = "Vale Gateway CLI tool for managing and inspecting gateway instances",
    version,
    long_about = "A command-line tool for discovering, managing, and debugging Vale Gateway instances in Kubernetes clusters."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Kubernetes namespace to operate in
    #[arg(short, long, global = true)]
    pub namespace: Option<String>,

    /// Kubeconfig file path
    #[arg(long, global = true)]
    pub kubeconfig: Option<String>,

    /// Output format
    #[arg(short, long, global = true, default_value = "table")]
    pub output: OutputFormat,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Use emojis in table output for better visual representation
    #[arg(long, global = true)]
    pub emoji: bool,
}

#[derive(Clone, clap::ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
    Yaml,
    Wide,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Get information about gateway resources
    Get {
        #[command(subcommand)]
        resource: GetResource,
    },
    /// Describe detailed information about gateway resources
    Describe {
        #[command(subcommand)]
        resource: DescribeResource,
    },
    /// Show logs from gateway components
    Logs {
        /// Gateway instance name
        name: String,
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
        /// Show logs from the last N lines
        #[arg(long, default_value = "100")]
        tail: i64,
        /// Show logs since duration (e.g., 5s, 2m, 3h)
        #[arg(long)]
        since: Option<String>,
        /// Container name (for multi-container pods)
        #[arg(short, long)]
        container: Option<String>,
    },
    /// Analyze gateway configuration and status
    Analyze {
        /// Gateway instance name (optional, analyzes all if not specified)
        name: Option<String>,
        /// Show configuration details
        #[arg(long)]
        config: bool,
        /// Show listener analysis
        #[arg(long)]
        listeners: bool,
        /// Show route analysis
        #[arg(long)]
        routes: bool,
    },
    /// Show current Kubernetes context information
    Context,
    /// Port forward to gateway instances
    #[command(name = "port-forward")]
    PortForward {
        /// Gateway instance name
        name: String,
        /// Local port
        local_port: u16,
        /// Remote port (defaults to 8080)
        #[arg(default_value = "8080")]
        remote_port: u16,
    },
    /// Execute commands in gateway pods
    Exec {
        /// Gateway instance name
        name: String,
        /// Container name
        #[arg(short, long)]
        container: Option<String>,
        /// Command to execute
        #[arg(last = true)]
        command: Vec<String>,
    },
    /// Show status of gateway resources
    Status {
        /// Resource type to show status for
        #[arg(value_enum)]
        resource_type: crate::commands::status::StatusResourceType,
        /// Resource name (optional)
        name: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum GetResource {
    /// List gateway instances
    Gateways {
        /// Gateway name (optional)
        name: Option<String>,
        /// Show all namespaces
        #[arg(short = 'A', long)]
        all_namespaces: bool,
        /// Label selector
        #[arg(short, long)]
        selector: Option<String>,
    },
    /// List gateway classes
    #[command(name = "gateway-classes")]
    GatewayClasses {
        /// Gateway class name (optional)
        name: Option<String>,
    },
    /// List pods associated with gateways
    Pods {
        /// Gateway instance name (optional)
        gateway: Option<String>,
        /// Show all namespaces
        #[arg(short = 'A', long)]
        all_namespaces: bool,
    },
    /// List services associated with gateways
    Services {
        /// Gateway instance name (optional)
        gateway: Option<String>,
        /// Show all namespaces
        #[arg(short = 'A', long)]
        all_namespaces: bool,
    },
    /// List deployments associated with gateways
    Deployments {
        /// Gateway instance name (optional)
        gateway: Option<String>,
        /// Show all namespaces
        #[arg(short = 'A', long)]
        all_namespaces: bool,
    },
}

#[derive(Subcommand)]
pub enum DescribeResource {
    /// Describe gateway instances
    Gateway {
        /// Gateway name
        name: String,
    },
    /// Describe gateway pods
    Pod {
        /// Pod name
        name: String,
    },
}
