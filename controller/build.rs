include!("src/api/v1alpha1.rs");

use kube::CustomResourceExt;
use std::env;
use std::fs;
use std::fs::create_dir_all;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Define the target directory
    let out_dir = env::var("OUT_DIR")?;
    let crds_dir = Path::new(&out_dir).join("../../..").join("crds");

    [
        (GatewayDeployment::crd(), "gateway-deployment.yaml"),
        (GatewayService::crd(), "gateway-service.yaml"),
    ]
    .iter()
    .for_each(|(crd, file_name)| {
        let crds_dir = crds_dir.join(&crd.spec.versions.first().unwrap().name);
        create_dir_all(&crds_dir).unwrap();
        let target_path = crds_dir.join(file_name);

        let yaml = serde_yaml::to_string(crd).expect("Failed to serialize CRD");
        fs::write(&target_path, yaml).expect("Failed to write CRD to file");
    });

    println!("cargo:rerun-if-changed=build.rs");
    Ok(())
}
