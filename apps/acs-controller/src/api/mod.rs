use axum::{
    routing::{get, post, put},
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
        // ── Devices ─────────────────────────────────────────────────────────
        .route("/api/v1/inventory/devices",
            get(inventory::list_devices))
        .route("/api/v1/inventory/devices/:uid",
            get(inventory::get_device)
            .patch(inventory::patch_device)
            .delete(inventory::delete_device))
        // ── Device properties ────────────────────────────────────────────────
        .route("/api/v1/inventory/devices/:uid/properties",
            get(inventory::list_device_properties))
        .route("/api/v1/inventory/devices/:uid/properties/:name",
            put(inventory::set_device_property)
            .delete(inventory::delete_device_property))
        // ── Device protocols (read-only) ─────────────────────────────────────
        .route("/api/v1/inventory/devices/:uid/protocols",
            get(inventory::list_device_protocols))
        // ── Domains ──────────────────────────────────────────────────────────
        .route("/api/v1/inventory/domains",
            get(inventory::list_domains)
            .post(inventory::create_domain))
        .route("/api/v1/inventory/domains/:slug",
            get(inventory::get_domain)
            .patch(inventory::patch_domain)
            .delete(inventory::delete_domain))
        // ── Device commands ──────────────────────────────────────────────────
        .route("/api/v1/device/:uid/command",
            post(device::send_command))
        .layer(cors)
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("Starting HTTP API server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}
