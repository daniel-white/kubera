mod cli;
mod controllers;

pub mod ipc;
pub mod kubernetes;

use crate::controllers::{SpawnControllersParams, spawn_controllers};
use crate::ipc::{SpawnIpcParameters, spawn_ipc};
use crate::kubernetes::start_kubernetes_client;
use clap::Parser;
use cli::Cli;
use kubera_core::crypto::init_crypto;
use kubera_core::instrumentation::init_instrumentation;
use std::sync::Arc;
use tokio::task::JoinSet;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = Cli::parse();

    init_crypto();
    init_instrumentation();

    let mut join_set = JoinSet::new();

    let kube_client = start_kubernetes_client(&mut join_set);

    // IPC is half 1 - it is what the gateway use to ensure that they have the latest configuration
    let ipc_services = {
        let params = SpawnIpcParameters::new_builder()
            .port(args.port())
            .build()
            .expect("Failed to build IPC parameters");

        spawn_ipc(&mut join_set, params)
    };

    // Controllers are half 2 - they are responsible for ensuring that the configuration is up to date
    // through various controllers
    {
        let params = SpawnControllersParams::new_builder()
            .kube_client(kube_client)
            .ipc_services(Arc::new(ipc_services))
            .pod_namespace(args.pod_namespace())
            .pod_name(args.pod_name())
            .instance_name(args.instance_name())
            .build()
            .expect("Failed to build controller run parameters");
        spawn_controllers(&mut join_set, params);
    }

    join_set.join_all().await;
}
