use kube::CustomResourceExt;
use kubera_api::v1alpha1::*;
use kubera_core::config::gateway::types::GatewayConfiguration;
use schemars::schema_for;
use std::fs::create_dir_all;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_dir = Path::new(manifest_dir.as_str());
    let profile = std::env::var("PROFILE").unwrap();
    let out_dir = manifest_dir.join("..").join("target").join(profile);
    let out_dir = out_dir.as_path();

    write_gateway_configuration_schema(out_dir);

    let out_dir = manifest_dir
        .join("..")
        .join("helm")
        .join("crds")
        .join("generated");

    let out_dir = out_dir.as_path();
    write_crds(out_dir);
}

fn write_gateway_configuration_schema(out_dir: &Path) {
    create_dir_all(out_dir).unwrap();
    let dest_path = out_dir.join("gateway_configuration_schema.yaml");
    let mut file = File::create(dest_path).unwrap();

    let schema = schema_for!(GatewayConfiguration);
    file.write_all(serde_yaml::to_string(&schema).unwrap().as_bytes())
        .unwrap();
}

fn write_crds(out_dir: &Path) {
    create_dir_all(out_dir).unwrap();
    let dest_path = out_dir.join("crds.yaml");
    let file = File::create(dest_path).unwrap();

    [GatewayClassParameters::crd(), GatewayParameters::crd()]
        .iter()
        .fold(file, |mut output, crd| {
            writeln!(output, "---").unwrap();
            writeln!(output, "{}", serde_yaml::to_string(crd).unwrap().as_str()).unwrap();
            output
        });
}
