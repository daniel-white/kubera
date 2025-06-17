use kube::CustomResourceExt;
use kubera_api::v1alpha1::*;
use kubera_core::config::gateway::types::GatewayConfiguration;
use schemars::schema_for;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    let out_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = out_dir.as_str();

    write_gateway_configuration_schema(out_dir);
    write_crds(out_dir);
}

fn write_gateway_configuration_schema(out_dir: &str) {
    let dest_path = Path::new(&out_dir).join("gateway_configuration_schema.yaml");
    let mut file = File::create(dest_path).unwrap();
    
    let schema = schema_for!(GatewayConfiguration);
    file.write_all(serde_yaml::to_string(&schema).unwrap().as_bytes())
        .unwrap();
}

fn write_crds(out_dir: &str) {
    let dest_path = Path::new(out_dir).join("crds.yaml");
    let file = File::create(dest_path).unwrap();
    
    [GatewayClassParameters::crd(), GatewayParameters::crd()]
        .iter()
        .fold(file, |mut output, crd| {
            writeln!(output, "---").unwrap();
            writeln!(output, "{}", serde_yaml::to_string(crd).unwrap().as_str()).unwrap();
            output
        });
}
