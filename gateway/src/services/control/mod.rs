use axum::Router;
use kubera_core::net::Port;
use tokio::net::TcpListener;
use tokio::spawn;
use tracing::info;

pub async fn spawn_control_server(port: Port) {
    spawn(async move {
        info!("🚀 Starting control service on port port {}", port);

        let router = Router::new();

        let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
            .await
            .expect("🔌 Unable to bind control service address");
        axum::serve(listener, router)
            .await
            .expect("🛑 Failed to start control service");
    });
}
