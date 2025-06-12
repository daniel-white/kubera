// core/build.rs
use schemars::schema_for;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use kubera_core::config::gateway::types::GatewayConfiguration;

fn main() {
    // Generate schemas
    let config_schema = schema_for!(GatewayConfiguration);
    // let host_schema = schema_for!(Host);
    //
    // // Combine schemas into a single JSON object
    // let combined = serde_json::json!({
    //     "Config": config_schema,
    //     "Host": host_schema,
    // });

    // Write to file
    let out_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("config-schema.yaml");
    let mut file = File::create(dest_path).unwrap();
    file.write_all(serde_yaml::to_string(&config_schema).unwrap().as_bytes())
        .unwrap();
}
