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
    let session_id = match state.active_sessions.get(&uid) {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                "Device is not currently connected (no active session)",
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
