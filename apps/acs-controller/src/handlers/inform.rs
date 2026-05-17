//! Handler for `inform` events received from protocol pods.
//!
//! An `inform` event represents a device announcing itself to the ACS —
//! the equivalent of a CWMP Inform or a USP Notify. The controller reacts
//! by ensuring the device exists in the database and that its observable
//! state (versions, protocol, timestamps) is current.

use anyhow::Context;
use tracing::{debug, info};
use uuid::Uuid;

use crate::db::{self, InformPayload};

/// Handle a raw `inform` event payload received from a protocol pod.
///
/// Deserialises the JSON payload, logs key fields, then delegates to
/// [`db::upsert_device`] to persist the device state.
pub async fn handle_inform(
    raw: &[u8],
    pool: &sqlx::PgPool,
    default_domain_id: Uuid,
) -> anyhow::Result<()> {
    let payload: InformPayload =
        serde_json::from_slice(raw).context("Failed to deserialise InformPayload")?;

    info!(
        device_id  = %payload.device_id,
        session_id = %payload.session_id,
        protocol   = payload.effective_protocol(),
        oui        = %payload.oui,
        serial     = %payload.serial_number,
        events     = ?payload.events,
        "Inform received — upserting device",
    );

    db::upsert_device(pool, &payload, default_domain_id)
        .await
        .context("Failed to upsert device in database")?;

    debug!(
        device_id        = %payload.device_id,
        software_version = ?payload.software_version(),
        hardware_version = ?payload.hardware_version(),
        "Device upserted successfully",
    );

    Ok(())
}
