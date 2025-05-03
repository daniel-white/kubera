pub mod v1alpha1;

use anyhow::{Context, Result};
use kube::CustomResourceExt;
use std::fmt::Write;
use std::{
    fs::{create_dir_all, write},
    path::Path,
};
use v1alpha1::{GatewayDeployment, GatewayService};

pub fn write_crds(output_path: Option<&str>) -> Result<()> {
    let output = [GatewayDeployment::crd(), GatewayService::crd()]
        .iter()
        .fold(String::new(), |mut output, crd| {
            writeln!(output, "---").unwrap();
            writeln!(output, "{}", serde_yaml::to_string(crd).unwrap().as_str()).unwrap();
            output
        });
    let output = output.as_str();

    match output_path {
        None => {
            println!("{output}");
            Ok(())
        }
        Some(output_path) => {
            let output_path = Path::new(output_path);
            let dir = output_path
                .parent()
                .context("Failed to get parent directory")?;

            if !dir.exists() {
                create_dir_all(dir)
                    .with_context(|| format!("Unable to create directory: {}", dir.display()))?;
            }
            write(output_path, output)
                .with_context(|| format!("Unable to write CRDs to: {}", output_path.display()))?;

            Ok(())
        }
    }
}
