use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::api::state::ApiState;

// Simple struct for JSON responses
#[derive(Serialize, sqlx::FromRow)]
pub struct DeviceInfo {
    pub device_uid: String,
    pub domain_id: Uuid,
    pub manufacturer: String,
    pub oui: String,
    pub product_class: String,
    pub serial_number: String,
    pub current_protocol: Option<String>,
    pub software_version: Option<String>,
    pub hardware_version: Option<String>,
    pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
}

// Struct for returning domains
#[derive(Serialize, sqlx::FromRow)]
pub struct DomainInfo {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
}

pub async fn list_devices(State(state): State<ApiState>) -> impl IntoResponse {
    let rows = sqlx::query_as::<_, DeviceInfo>(
        r#"
        SELECT
            device_uid,
            domain_id,
            manufacturer,
            oui,
            product_class,
            serial_number,
            current_protocol,
            software_version,
            hardware_version,
            last_seen
        FROM devices
        "#
    )
    .fetch_all(&state.pool)
    .await;

    match rows {
        Ok(devices) => (StatusCode::OK, Json(devices)).into_response(),
        Err(e) => {
            tracing::error!(?e, "Failed to fetch devices");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

pub async fn get_device(
    State(state): State<ApiState>,
    Path(uid): Path<String>,
) -> impl IntoResponse {
    let row = sqlx::query_as::<_, DeviceInfo>(
        r#"
        SELECT
            device_uid,
            domain_id,
            manufacturer,
            oui,
            product_class,
            serial_number,
            current_protocol,
            software_version,
            hardware_version,
            last_seen
        FROM devices
        WHERE device_uid = $1
        "#
    )
    .bind(uid.clone())
    .fetch_optional(&state.pool)
    .await;

    match row {
        Ok(Some(device)) => (StatusCode::OK, Json(device)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "Device not found").into_response(),
        Err(e) => {
            tracing::error!(?e, uid, "Failed to fetch device");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

pub async fn list_domains(State(state): State<ApiState>) -> impl IntoResponse {
    let rows = sqlx::query_as::<_, DomainInfo>(
        r#"
        SELECT id, name, slug
        FROM domains
        "#
    )
    .fetch_all(&state.pool)
    .await;

    match rows {
        Ok(domains) => (StatusCode::OK, Json(domains)).into_response(),
        Err(e) => {
            tracing::error!(?e, "Failed to fetch domains");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}
