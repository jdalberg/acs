//! Handler for `inform` events received from protocol pods.
//!
//! An `inform` event represents a device announcing itself to the ACS —
//! the equivalent of a CWMP Inform or a USP Notify. The controller reacts
//! by ensuring the device exists in the database and that its observable
//! state (versions, protocol, timestamps) is current.

use anyhow::Context;
use nats_common::DeviceCommand;
use tracing::{debug, error, info};

use crate::db::{self, InformPayload};
use crate::nats::NatsClient;
use crate::Config;
use crate::provisioning;

/// Handle a raw `inform` event payload received from a protocol pod.
///
/// Deserialises the JSON payload, logs key fields, then delegates to
/// [`db::upsert_device`] to persist the device state. Then it executes
/// provisioning scripts for the "inform" event and publishes resulting
/// commands to NATS.
pub async fn handle_inform(
    raw: &[u8],
    pool: &sqlx::PgPool,
    nats: &NatsClient,
    config: &Config,
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

    db::upsert_device(pool, &payload, config.default_domain_id)
        .await
        .context("Failed to upsert device in database")?;

    debug!(
        device_id        = %payload.device_id,
        software_version = ?payload.software_version(),
        hardware_version = ?payload.hardware_version(),
        "Device upserted successfully",
    );

    let domain_slug = db::get_domain_slug(pool, config.default_domain_id)
        .await
        .context("Failed to fetch domain slug for provisioning")?;

    // Execute provisioning scripts
    let actions = provisioning::run_scripts(
        &config.provisioning_root,
        &domain_slug,
        payload.hardware_version(),
        payload.software_version(),
        &payload.serial_number,
        "inform",
        raw,
    )
    .await
    .context("Provisioning engine failed")?;

    if !actions.is_empty() {
        info!("Publishing {} commands from provisioning scripts", actions.len());
        for action in actions {
            let command = DeviceCommand {
                command_id: uuid::Uuid::new_v4(),
                device_id: payload.device_id.clone(),
                action,
            };

            let cmd_payload = serde_json::to_vec(&command)
                .context("Failed to serialize DeviceCommand")?;
            
            if let Err(e) = nats.publish_command(&payload.session_id, cmd_payload).await {
                error!(?e, session_id = %payload.session_id, "Failed to publish DeviceCommand to NATS");
            } else {
                debug!(command_id = %command.command_id, "Published command successfully");
            }
        }
    }

    Ok(())
}
