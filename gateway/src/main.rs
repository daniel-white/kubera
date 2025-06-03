//use pingora::gateway::{Proxy, ProxyConfig};
//use tokio::runtime::Runtime;

mod http;
mod request_matcher;

#[tokio::main]
async fn main() {
    // // Create a runtime for the async operations
    // let rt = Runtime::new().expect("Failed to create Tokio runtime");

    // rt.block_on(async {
    //     // Define the gateway configuration
    //     let config = ProxyConfig {
    //         listen_addr: "127.0.0.1:3000".parse().expect("Invalid listen address"),
    //         backend_addr: "http://example.com"
    //             .parse()
    //             .expect("Invalid backend address"),
    //         ..Default::default()
    //     };

    //     // Create and run the gateway
    //     let gateway = Proxy::new(config);
    //     println!("Pingora gateway running on http://127.0.0.1:3000");

    //     if let Err(e) = gateway.run().await {
    //         eprintln!("Proxy error: {}", e);
    //     }
    // });
}
