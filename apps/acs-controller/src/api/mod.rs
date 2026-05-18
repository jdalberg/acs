use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

pub mod device;
pub mod inventory;
pub mod state;

pub use state::ApiState;

pub async fn start_server(state: ApiState, port: u16) -> Result<(), std::io::Error> {
    // Enable CORS for GUI development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/v1/inventory/devices", get(inventory::list_devices))
        .route("/api/v1/inventory/devices/:uid", get(inventory::get_device))
        .route("/api/v1/inventory/domains", get(inventory::list_domains))
        .route("/api/v1/device/:uid/command", post(device::send_command))
        .layer(cors)
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("Starting HTTP API server on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}
