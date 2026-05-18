use std::time::Duration;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use nats_common::{Action, DeviceCommand, DeviceResponse};
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::api::state::ApiState;

pub async fn send_command(
    State(state): State<ApiState>,
    Path(uid): Path<String>,
    Json(action): Json<Action>,
) -> impl IntoResponse {
    // 1. Check if the device is currently online
    let mut session_id_opt = state.active_sessions.get(&uid).map(|s| s.clone());

    if session_id_opt.is_none() {
        tracing::info!(%uid, "Device offline. Querying connection request details...");
        
        let row: Result<(Option<String>, Option<String>), sqlx::Error> = sqlx::query_as(
            r#"
            SELECT dp.connection_request_url, dp.username
            FROM device_protocols dp
            JOIN devices d ON dp.device_id = d.id
            WHERE d.device_uid = $1 AND dp.protocol = 'cwmp'
            "#,
        )
        .bind(&uid)
        .fetch_one(&state.pool)
        .await;

        match row {
            Ok((Some(url), username)) => {
                let payload = serde_json::json!({
                    "device_id": uid,
                    "connection_request_url": url,
                    "username": username,
                    "password": Option::<String>::None // To be implemented if we store passwords
                });
                
                if let Ok(bytes) = serde_json::to_vec(&payload) {
                    if let Err(e) = state.nats.publish_connection_request(bytes).await {
                        tracing::error!(?e, "Failed to publish connection request");
                    } else {
                        tracing::info!(%uid, "Published connection request, waiting for device...");
                        
                        // Poll for up to 15 seconds
                        let timeout = tokio::time::Instant::now() + Duration::from_secs(15);
                        while tokio::time::Instant::now() < timeout {
                            if let Some(s) = state.active_sessions.get(&uid) {
                                session_id_opt = Some(s.clone());
                                break;
                            }
                            tokio::time::sleep(Duration::from_millis(250)).await;
                        }
                    }
                }
            }
            Ok((None, _)) => {
                tracing::warn!(%uid, "Device found but no connection request URL recorded.");
            }
            Err(e) => {
                tracing::warn!(?e, %uid, "Failed to fetch connection request details.");
            }
        }
    }

    let session_id = match session_id_opt {
        Some(s) => s,
        None => {
            return (
                StatusCode::GATEWAY_TIMEOUT,
                "Device is not currently connected and failed to wake up",
            )
                .into_response();
        }
    };


    // 2. Prepare the command and a oneshot channel to await the response
    let command_id = Uuid::new_v4();
    let command = DeviceCommand {
        command_id,
        device_id: uid.clone(),
        action,
    };

    let (tx, rx) = oneshot::channel::<DeviceResponse>();
    state.pending_commands.insert(command_id, tx);

    // 3. Serialize and publish the command to NATS
    let payload = match serde_json::to_vec(&command) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(?e, "Failed to serialize DeviceCommand");
            state.pending_commands.remove(&command_id);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to serialize command",
            )
                .into_response();
        }
    };

    if let Err(e) = state.nats.publish_command(&session_id, payload).await {
        tracing::error!(?e, %session_id, "Failed to publish command to NATS");
        state.pending_commands.remove(&command_id);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to publish command to NATS",
        )
            .into_response();
    }

    tracing::info!(%uid, %session_id, %command_id, "Command published, awaiting response");

    // 4. Await the response with a timeout
    match tokio::time::timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(response)) => {
            // Received response from device
            (StatusCode::OK, Json(response)).into_response()
        }
        Ok(Err(_)) => {
            // The sender was dropped (e.g. session ended unexpectedly)
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Session ended before response was received",
            )
                .into_response()
        }
        Err(_) => {
            // Timeout
            state.pending_commands.remove(&command_id);
            (StatusCode::GATEWAY_TIMEOUT, "Device response timeout").into_response()
        }
    }
}
