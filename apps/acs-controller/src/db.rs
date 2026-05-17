//! Database helpers for the acs-controller.
//!
//! All SQL is written inline here for transparency and easy diffing against
//! the `.sql` files in `db/`. If compile-time query checking is desired,
//! run `cargo sqlx prepare` against a live database to generate the
//! `.sqlx/` offline cache, then add `SQLX_OFFLINE=true` to your build env.

use std::collections::HashMap;

use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

// ── Payload types ─────────────────────────────────────────────────────────────

/// Typed representation of the `inform` event payload published by protocol pods.
///
/// Mirrors the JSON object emitted by `acs-cwmp::handlers::handle_inform_post`.
/// Any additional protocol pod must publish the same shape.
#[derive(Debug, Deserialize)]
pub struct InformPayload {
    pub session_id:     String,
    /// Composite device key: `"{oui}-{serial_number}"` — maps to `device_uid` in DB.
    pub device_id:      String,
    pub oui:            String,
    pub serial_number:  String,
    pub manufacturer:   String,
    pub product_class:  String,
    /// Raw CWMP/USP event strings, e.g. `["1 BOOT", "0 BOOTSTRAP"]`.
    pub events:         Vec<String>,
    /// TR-181 / TR-098 parameter paths → values as reported in the Inform.
    pub parameter_list: HashMap<String, String>,
    /// Protocol that delivered this event ("cwmp", "usp", …).
    /// Defaults to `"cwmp"` for backward compatibility with pods that predate
    /// the field.
    #[serde(default)]
    pub protocol:       Option<String>,
}

impl InformPayload {
    /// Best-effort software version extracted from the parameter list.
    /// Tries both TR-181 and TR-098 paths.
    pub fn software_version(&self) -> Option<&str> {
        self.parameter_list
            .get("Device.DeviceInfo.SoftwareVersion")
            .or_else(|| self.parameter_list.get("InternetGatewayDevice.DeviceInfo.SoftwareVersion"))
            .map(String::as_str)
    }

    /// Best-effort hardware version extracted from the parameter list.
    pub fn hardware_version(&self) -> Option<&str> {
        self.parameter_list
            .get("Device.DeviceInfo.HardwareVersion")
            .or_else(|| self.parameter_list.get("InternetGatewayDevice.DeviceInfo.HardwareVersion"))
            .map(String::as_str)
    }

    /// The protocol string to store, defaulting to `"cwmp"` when absent.
    pub fn effective_protocol(&self) -> &str {
        self.protocol.as_deref().unwrap_or("cwmp")
    }
}

// ── Database operations ────────────────────────────────────────────────────────

/// Upsert a device row from an Inform payload.
///
/// ## First contact
/// Inserts a new device row assigned to `default_domain_id`.
///
/// ## Subsequent Informs
/// Updates `last_seen` and all observable fields.
/// `software_version` and `hardware_version` use `COALESCE` so a sparse
/// Inform that omits these parameters never overwrites a previously known
/// good value.
pub async fn upsert_device(
    pool: &PgPool,
    payload: &InformPayload,
    default_domain_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO devices (
            domain_id,
            device_uid,
            manufacturer,
            oui,
            product_class,
            serial_number,
            current_protocol,
            software_version,
            hardware_version
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (domain_id, device_uid) DO UPDATE SET
            last_seen        = now(),
            manufacturer     = EXCLUDED.manufacturer,
            oui              = EXCLUDED.oui,
            product_class    = EXCLUDED.product_class,
            serial_number    = EXCLUDED.serial_number,
            current_protocol = EXCLUDED.current_protocol,
            software_version = COALESCE(EXCLUDED.software_version, devices.software_version),
            hardware_version = COALESCE(EXCLUDED.hardware_version, devices.hardware_version)
        "#,
    )
    .bind(default_domain_id)
    .bind(&payload.device_id)       // device_uid = "{oui}-{serial}"
    .bind(&payload.manufacturer)
    .bind(&payload.oui)
    .bind(&payload.product_class)
    .bind(&payload.serial_number)
    .bind(payload.effective_protocol())
    .bind(payload.software_version()) // Option<&str> — NULL if not in parameter_list
    .bind(payload.hardware_version()) // Option<&str> — NULL if not in parameter_list
    .execute(pool)
    .await?;

    Ok(())
}
