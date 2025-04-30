//use pingora::proxy::{Proxy, ProxyConfig};
//use tokio::runtime::Runtime;

fn main() {
    // // Create a runtime for the async operations
    // let rt = Runtime::new().expect("Failed to create Tokio runtime");

    // rt.block_on(async {
    //     // Define the proxy configuration
    //     let config = ProxyConfig {
    //         listen_addr: "127.0.0.1:3000".parse().expect("Invalid listen address"),
    //         backend_addr: "http://example.com"
    //             .parse()
    //             .expect("Invalid backend address"),
    //         ..Default::default()
    //     };

    //     // Create and run the proxy
    //     let proxy = Proxy::new(config);
    //     println!("Pingora proxy running on http://127.0.0.1:3000");

    //     if let Err(e) = proxy.run().await {
    //         eprintln!("Proxy error: {}", e);
    //     }
    // });
}
