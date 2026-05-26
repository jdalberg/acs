//! Inventory management API handlers.
//!
//! Provides a complete CRUD surface for devices, device properties,
//! device protocols, and domains. All writes are scoped to what the
//! Inform flow creates; `POST /devices` is intentionally absent —
//! devices are registered automatically on first contact.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::JsonValue;
use uuid::Uuid;

use crate::api::state::ApiState;

// ── Response types ────────────────────────────────────────────────────────────

/// Full device record returned by the API.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DeviceInfo {
    pub id:               Uuid,
    pub device_uid:       String,
    pub domain_id:        Uuid,
    pub manufacturer:     Option<String>,
    pub oui:              Option<String>,
    pub product_class:    Option<String>,
    pub serial_number:    Option<String>,
    pub current_protocol: Option<String>,
    pub software_version: Option<String>,
    pub hardware_version: Option<String>,
    pub tags:             Vec<String>,
    pub metadata:         JsonValue,
    pub first_seen:       chrono::DateTime<chrono::Utc>,
    pub last_seen:        chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DomainInfo {
    pub id:          Uuid,
    pub name:        String,
    pub slug:        String,
    pub description: Option<String>,
    pub created_at:  chrono::DateTime<chrono::Utc>,
    pub updated_at:  chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DeviceProperty {
    pub property_name:  String,
    pub property_value: JsonValue,
    pub source:         String,
    pub priority:       i32,
    pub created_at:     chrono::DateTime<chrono::Utc>,
    pub updated_at:     chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DeviceProtocol {
    pub protocol:              String,
    pub endpoint_id:           Option<String>,
    pub connection_request_url:Option<String>,
    pub username:              Option<String>,
    pub last_session_at:       Option<chrono::DateTime<chrono::Utc>>,
    pub metadata:              JsonValue,
}

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct DeviceListQuery {
    /// Filter by domain slug, e.g. `?domain=acme`
    pub domain: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PatchDeviceRequest {
    pub tags:      Option<Vec<String>>,
    pub metadata:  Option<Value>,
    pub domain_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDomainRequest {
    pub name:        String,
    pub slug:        String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PatchDomainRequest {
    pub name:        Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetPropertyRequest {
    pub value:    Value,
    pub source:   Option<String>,
    pub priority: Option<i32>,
}

// ── Devices ───────────────────────────────────────────────────────────────────

const DEVICE_SELECT: &str = r#"
    SELECT
        d.id, d.device_uid, d.domain_id, d.manufacturer, d.oui,
        d.product_class, d.serial_number, d.current_protocol,
        d.software_version, d.hardware_version, d.tags, d.metadata,
        d.first_seen, d.last_seen
    FROM devices d
"#;

/// `GET /api/v1/inventory/devices[?domain=<slug>]`
pub async fn list_devices(
    State(state): State<ApiState>,
    Query(params): Query<DeviceListQuery>,
) -> impl IntoResponse {
    let result = match params.domain {
        None => {
            sqlx::query_as::<_, DeviceInfo>(&format!("{DEVICE_SELECT} ORDER BY d.last_seen DESC"))
                .fetch_all(&state.pool)
                .await
        }
        Some(slug) => {
            sqlx::query_as::<_, DeviceInfo>(&format!(
                "{DEVICE_SELECT} JOIN domains dom ON dom.id = d.domain_id \
                 WHERE dom.slug = $1 ORDER BY d.last_seen DESC"
            ))
            .bind(slug)
            .fetch_all(&state.pool)
            .await
        }
    };

    match result {
        Ok(devices) => (StatusCode::OK, Json(devices)).into_response(),
        Err(e) => {
            tracing::error!(?e, "list_devices: db error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

/// `GET /api/v1/inventory/devices/:uid`
pub async fn get_device(
    State(state): State<ApiState>,
    Path(uid): Path<String>,
) -> impl IntoResponse {
    let result = sqlx::query_as::<_, DeviceInfo>(&format!("{DEVICE_SELECT} WHERE d.device_uid = $1"))
        .bind(&uid)
        .fetch_optional(&state.pool)
        .await;

    match result {
        Ok(Some(device)) => (StatusCode::OK, Json(device)).into_response(),
        Ok(None)         => (StatusCode::NOT_FOUND, "Device not found").into_response(),
        Err(e) => {
            tracing::error!(?e, uid, "get_device: db error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

/// `PATCH /api/v1/inventory/devices/:uid`
///
/// Updates `tags`, `metadata`, and/or `domain_id`. Only supplied fields are changed.
pub async fn patch_device(
    State(state): State<ApiState>,
    Path(uid): Path<String>,
    Json(body): Json<PatchDeviceRequest>,
) -> impl IntoResponse {
    // Resolve device UUID from uid first
    let device_uuid: Option<Uuid> = sqlx::query_scalar("SELECT id FROM devices WHERE device_uid = $1")
        .bind(&uid)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or(None);

    let Some(device_uuid) = device_uuid else {
        return (StatusCode::NOT_FOUND, "Device not found").into_response();
    };

    // Build update dynamically: only touch columns the caller provided
    if body.tags.is_none() && body.metadata.is_none() && body.domain_id.is_none() {
        return (StatusCode::BAD_REQUEST, "Nothing to update").into_response();
    }

    let mut sets: Vec<String> = Vec::new();
    let mut idx: i32 = 1;

    if body.tags.is_some()      { sets.push(format!("tags = ${idx}"));      idx += 1; }
    if body.metadata.is_some()  { sets.push(format!("metadata = ${idx}"));  idx += 1; }
    if body.domain_id.is_some() { sets.push(format!("domain_id = ${idx}")); idx += 1; }

    let sql = format!(
        "UPDATE devices SET {} WHERE id = ${} RETURNING id",
        sets.join(", "),
        idx
    );

    let mut q = sqlx::query_scalar::<_, Uuid>(&sql);
    if let Some(ref tags)      = body.tags      { q = q.bind(tags); }
    if let Some(ref metadata)  = body.metadata  { q = q.bind(metadata); }
    if let Some(domain_id)     = body.domain_id { q = q.bind(domain_id); }
    q = q.bind(device_uuid);

    match q.fetch_optional(&state.pool).await {
        Ok(Some(_)) => StatusCode::NO_CONTENT.into_response(),
        Ok(None)    => (StatusCode::NOT_FOUND, "Device not found").into_response(),
        Err(e) => {
            tracing::error!(?e, uid, "patch_device: db error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

/// `DELETE /api/v1/inventory/devices/:uid`
///
/// Hard-deletes the device. Cascade FK rules remove protocols and properties.
pub async fn delete_device(
    State(state): State<ApiState>,
    Path(uid): Path<String>,
) -> impl IntoResponse {
    let result: Result<Option<Uuid>, sqlx::Error> =
        sqlx::query_scalar("DELETE FROM devices WHERE device_uid = $1 RETURNING id")
            .bind(&uid)
            .fetch_optional(&state.pool)
            .await;

    match result {
        Ok(Some(_)) => StatusCode::NO_CONTENT.into_response(),
        Ok(None)    => (StatusCode::NOT_FOUND, "Device not found").into_response(),
        Err(e) => {
            tracing::error!(?e, uid, "delete_device: db error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

// ── Device properties ─────────────────────────────────────────────────────────

/// `GET /api/v1/inventory/devices/:uid/properties`
pub async fn list_device_properties(
    State(state): State<ApiState>,
    Path(uid): Path<String>,
) -> impl IntoResponse {
    let result = sqlx::query_as::<_, DeviceProperty>(
        r#"
        SELECT
            dp.property_name, dp.property_value, dp.source, dp.priority,
            dp.created_at, dp.updated_at
        FROM device_properties dp
        JOIN devices d ON d.id = dp.device_id
        WHERE d.device_uid = $1
        ORDER BY dp.property_name
        "#,
    )
    .bind(&uid)
    .fetch_all(&state.pool)
    .await;

    match result {
        Ok(props) => (StatusCode::OK, Json(props)).into_response(),
        Err(e) => {
            tracing::error!(?e, uid, "list_device_properties: db error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

/// `PUT /api/v1/inventory/devices/:uid/properties/:name`
pub async fn set_device_property(
    State(state): State<ApiState>,
    Path((uid, name)): Path<(String, String)>,
    Json(body): Json<SetPropertyRequest>,
) -> impl IntoResponse {
    let device_uuid: Option<Uuid> = sqlx::query_scalar("SELECT id FROM devices WHERE device_uid = $1")
        .bind(&uid)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or(None);

    let Some(device_uuid) = device_uuid else {
        return (StatusCode::NOT_FOUND, "Device not found").into_response();
    };

    let source   = body.source.unwrap_or_else(|| "api".to_string());
    let priority = body.priority.unwrap_or(100);

    let result = sqlx::query(
        r#"
        INSERT INTO device_properties (device_id, property_name, property_value, source, priority)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (device_id, property_name) DO UPDATE SET
            property_value = EXCLUDED.property_value,
            source         = EXCLUDED.source,
            priority       = EXCLUDED.priority,
            updated_at     = now()
        "#,
    )
    .bind(device_uuid)
    .bind(&name)
    .bind(&body.value)
    .bind(&source)
    .bind(priority)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_)  => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!(?e, uid, name, "set_device_property: db error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

/// `DELETE /api/v1/inventory/devices/:uid/properties/:name`
pub async fn delete_device_property(
    State(state): State<ApiState>,
    Path((uid, name)): Path<(String, String)>,
) -> impl IntoResponse {
    let result: Result<Option<String>, sqlx::Error> = sqlx::query_scalar(
        r#"
        DELETE FROM device_properties dp
        USING devices d
        WHERE dp.device_id = d.id
          AND d.device_uid  = $1
          AND dp.property_name = $2
        RETURNING dp.property_name
        "#,
    )
    .bind(&uid)
    .bind(&name)
    .fetch_optional(&state.pool)
    .await;

    match result {
        Ok(Some(_)) => StatusCode::NO_CONTENT.into_response(),
        Ok(None)    => (StatusCode::NOT_FOUND, "Property not found").into_response(),
        Err(e) => {
            tracing::error!(?e, uid, name, "delete_device_property: db error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

// ── Device protocols (read-only) ──────────────────────────────────────────────

/// `GET /api/v1/inventory/devices/:uid/protocols`
pub async fn list_device_protocols(
    State(state): State<ApiState>,
    Path(uid): Path<String>,
) -> impl IntoResponse {
    let result = sqlx::query_as::<_, DeviceProtocol>(
        r#"
        SELECT
            dp.protocol, dp.endpoint_id, dp.connection_request_url,
            dp.username, dp.last_session_at, dp.metadata
        FROM device_protocols dp
        JOIN devices d ON d.id = dp.device_id
        WHERE d.device_uid = $1
        ORDER BY dp.protocol
        "#,
    )
    .bind(&uid)
    .fetch_all(&state.pool)
    .await;

    match result {
        Ok(protocols) => (StatusCode::OK, Json(protocols)).into_response(),
        Err(e) => {
            tracing::error!(?e, uid, "list_device_protocols: db error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

// ── Domains ───────────────────────────────────────────────────────────────────

/// `GET /api/v1/inventory/domains`
pub async fn list_domains(State(state): State<ApiState>) -> impl IntoResponse {
    let result = sqlx::query_as::<_, DomainInfo>(
        "SELECT id, name, slug, description, created_at, updated_at FROM domains ORDER BY name",
    )
    .fetch_all(&state.pool)
    .await;

    match result {
        Ok(domains) => (StatusCode::OK, Json(domains)).into_response(),
        Err(e) => {
            tracing::error!(?e, "list_domains: db error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

/// `GET /api/v1/inventory/domains/:slug`
pub async fn get_domain(
    State(state): State<ApiState>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    let result = sqlx::query_as::<_, DomainInfo>(
        "SELECT id, name, slug, description, created_at, updated_at FROM domains WHERE slug = $1",
    )
    .bind(&slug)
    .fetch_optional(&state.pool)
    .await;

    match result {
        Ok(Some(domain)) => (StatusCode::OK, Json(domain)).into_response(),
        Ok(None)         => (StatusCode::NOT_FOUND, "Domain not found").into_response(),
        Err(e) => {
            tracing::error!(?e, slug, "get_domain: db error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

/// `POST /api/v1/inventory/domains`
pub async fn create_domain(
    State(state): State<ApiState>,
    Json(body): Json<CreateDomainRequest>,
) -> impl IntoResponse {
    let result = sqlx::query_as::<_, DomainInfo>(
        r#"
        INSERT INTO domains (name, slug, description)
        VALUES ($1, $2, $3)
        RETURNING id, name, slug, description, created_at, updated_at
        "#,
    )
    .bind(&body.name)
    .bind(&body.slug)
    .bind(&body.description)
    .fetch_one(&state.pool)
    .await;

    match result {
        Ok(domain) => (StatusCode::CREATED, Json(domain)).into_response(),
        Err(e) if is_unique_violation(&e) => {
            (StatusCode::CONFLICT, "Domain name or slug already exists").into_response()
        }
        Err(e) if is_check_violation(&e) => {
            (StatusCode::UNPROCESSABLE_ENTITY, "Slug must be lowercase letters, digits, and hyphens only").into_response()
        }
        Err(e) => {
            tracing::error!(?e, "create_domain: db error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

/// `PATCH /api/v1/inventory/domains/:slug`
pub async fn patch_domain(
    State(state): State<ApiState>,
    Path(slug): Path<String>,
    Json(body): Json<PatchDomainRequest>,
) -> impl IntoResponse {
    if body.name.is_none() && body.description.is_none() {
        return (StatusCode::BAD_REQUEST, "Nothing to update").into_response();
    }

    let mut sets: Vec<String> = vec!["updated_at = now()".to_string()];
    let mut idx: i32 = 1;

    if body.name.is_some()        { sets.push(format!("name = ${idx}"));        idx += 1; }
    if body.description.is_some() { sets.push(format!("description = ${idx}")); idx += 1; }

    let sql = format!(
        "UPDATE domains SET {} WHERE slug = ${} RETURNING id",
        sets.join(", "),
        idx
    );

    let mut q = sqlx::query_scalar::<_, Uuid>(&sql);
    if let Some(ref name)        = body.name        { q = q.bind(name); }
    if let Some(ref description) = body.description { q = q.bind(description); }
    q = q.bind(&slug);

    match q.fetch_optional(&state.pool).await {
        Ok(Some(_)) => StatusCode::NO_CONTENT.into_response(),
        Ok(None)    => (StatusCode::NOT_FOUND, "Domain not found").into_response(),
        Err(e) if is_unique_violation(&e) => {
            (StatusCode::CONFLICT, "Domain name already taken").into_response()
        }
        Err(e) => {
            tracing::error!(?e, slug, "patch_domain: db error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

/// `DELETE /api/v1/inventory/domains/:slug`
///
/// Fails with 409 if any devices are still assigned to this domain
/// (enforced by `ON DELETE RESTRICT` on the `devices.domain_id` FK).
pub async fn delete_domain(
    State(state): State<ApiState>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    let result: Result<Option<Uuid>, sqlx::Error> =
        sqlx::query_scalar("DELETE FROM domains WHERE slug = $1 RETURNING id")
            .bind(&slug)
            .fetch_optional(&state.pool)
            .await;

    match result {
        Ok(Some(_)) => StatusCode::NO_CONTENT.into_response(),
        Ok(None)    => (StatusCode::NOT_FOUND, "Domain not found").into_response(),
        Err(e) if is_foreign_key_violation(&e) => {
            (StatusCode::CONFLICT, "Domain still has devices assigned to it").into_response()
        }
        Err(e) => {
            tracing::error!(?e, slug, "delete_domain: db error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

// ── DB error helpers ──────────────────────────────────────────────────────────

fn pg_code(e: &sqlx::Error) -> Option<String> {
    if let sqlx::Error::Database(db) = e {
        return db.code().map(|c| c.into_owned());
    }
    None
}

fn is_unique_violation(e: &sqlx::Error) -> bool       { pg_code(e).as_deref() == Some("23505") }
fn is_foreign_key_violation(e: &sqlx::Error) -> bool  { pg_code(e).as_deref() == Some("23503") }
fn is_check_violation(e: &sqlx::Error) -> bool        { pg_code(e).as_deref() == Some("23514") }
