//! Translation from protocol-agnostic [`DeviceCommand`] to CWMP SOAP XML.
//!
//! This module is the only place in `acs-cwmp` that knows about the CWMP wire
//! format for *outgoing* (ACS→CPE) messages. Everything above this layer
//! operates on [`nats_common::DeviceCommand`] / [`nats_common::Action`].
//!
//! # How correlation works
//!
//! The [`DeviceCommand::command_id`] UUID is used verbatim as the CWMP
//! `<ID mustUnderstand="1">` header value. The CPE echoes this ID back in every
//! response envelope, so when `handle_non_inform_post` receives the response it
//! can pass the ID up to the controller for correlation without any extra state.

use cwmp::protocol::{
    AddObject, BodyElement, DeleteObject, Download, Envelope, FactoryReset,
    GetParameterNames, GetParameterValues, HeaderElement, ParameterValue, Reboot,
    SetParameterValues, Upload, ID,
};
use nats_common::{Action, DeviceCommand};
use thiserror::Error;

/// Errors that can occur when translating a [`DeviceCommand`] to CWMP XML.
#[derive(Debug, Error)]
pub enum TranslateError {
    /// The action cannot be expressed in the CWMP protocol.
    #[error("action not supported by CWMP: {0}")]
    UnsupportedAction(String),

    /// XML serialisation failed.
    #[error("XML serialisation failed: {0}")]
    Serialization(String),
}

/// Convert a protocol-agnostic [`DeviceCommand`] into a CWMP SOAP XML string
/// ready to be sent as the HTTP response body to a CPE.
///
/// The `command_id` from the command is used as the CWMP `<ID>` header so the
/// CPE echoes it back in the response envelope, enabling correlation.
pub fn command_to_xml(cmd: &DeviceCommand) -> Result<String, TranslateError> {
    let cwmp_id = ID::new(true, &cmd.command_id.to_string());
    let body = action_to_body_element(&cmd.action)?;

    let envelope = Envelope {
        cwmp_version: None,
        header: vec![HeaderElement::ID(cwmp_id)],
        body: vec![body],
    };

    cwmp::generate(&envelope)
        .map_err(|e| TranslateError::Serialization(e.to_string()))
}

// ── Private helpers ────────────────────────────────────────────────────────────

fn action_to_body_element(action: &Action) -> Result<BodyElement, TranslateError> {
    match action {
        // ── GetParameterValues ────────────────────────────────────────────────
        Action::GetParameterValues { paths } => {
            let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
            Ok(BodyElement::GetParameterValues(GetParameterValues::new(
                &refs,
            )))
        }

        // ── GetParameterNames ─────────────────────────────────────────────────
        Action::GetParameterNames {
            path_prefix,
            next_level,
        } => Ok(BodyElement::GetParameterNames(GetParameterNames::new(
            path_prefix,
            if *next_level { 1 } else { 0 },
        ))),

        // ── SetParameterValues ────────────────────────────────────────────────
        Action::SetParameterValues { parameters } => {
            // Build owned ParameterValue objects first, then borrow them.
            let owned: Vec<ParameterValue> = parameters
                .iter()
                .map(|(k, v)| {
                    // Type defaults to "xsd:string" — the controller can
                    // override via the key convention "path|type" if needed
                    // in a future iteration.
                    ParameterValue::new(k, "xsd:string", v)
                })
                .collect();
            let refs: Vec<&ParameterValue> = owned.iter().collect();
            Ok(BodyElement::SetParameterValues(SetParameterValues::new(
                None, // ParameterKey — left empty; controller can set via future field
                &refs,
            )))
        }

        // ── AddObject ─────────────────────────────────────────────────────────
        Action::AddObject { path } => Ok(BodyElement::AddObject(AddObject::new(path, ""))),

        // ── DeleteObject ──────────────────────────────────────────────────────
        Action::DeleteObject { path } => {
            Ok(BodyElement::DeleteObject(DeleteObject::new(path, "")))
        }

        // ── Reboot ────────────────────────────────────────────────────────────
        Action::Reboot => Ok(BodyElement::Reboot(Reboot::new(""))),

        // ── FactoryReset ──────────────────────────────────────────────────────
        Action::FactoryReset => Ok(BodyElement::FactoryReset(FactoryReset)),

        // ── Download ──────────────────────────────────────────────────────────
        Action::Download {
            url,
            file_type,
            file_size,
            target_filename,
        } => Ok(BodyElement::Download(Download::new(
            "",              // CommandKey — empty for now
            file_type,       // FileType
            url,             // URL
            "",              // Username
            "",              // Password
            *file_size,      // FileSize
            target_filename, // TargetFileName
            0,               // DelaySeconds
            "",              // SuccessURL
            "",              // FailureURL
        ))),

        // ── Upload ────────────────────────────────────────────────────────────
        Action::Upload { url, file_type } => Ok(BodyElement::Upload(Upload::new(
            "",        // CommandKey
            file_type,
            url,
            "",        // Username
            "",        // Password
            0,         // DelaySeconds
        ))),
    }
}

/// Convert a CPE's response [`BodyElement`] into a protocol-agnostic
/// [`nats_common::DeviceResponse`] for the controller.
///
/// `command_id` is the UUID that was echoed back in the CWMP `<ID>` header of
/// the CPE's response envelope — this is how the controller correlates the
/// response to the original [`DeviceCommand`].
pub fn body_element_to_response(
    body_element: &BodyElement,
    command_id: Option<uuid::Uuid>,
    device_id: String,
) -> nats_common::DeviceResponse {
    use nats_common::{ActionResult, DeviceResponse};
    use std::collections::HashMap;

    let result = match body_element {
        // ── GetParameterValues response ───────────────────────────────────────
        BodyElement::GetParameterValuesResponse(r) => {
            let map: HashMap<String, String> = r
                .parameters
                .iter()
                .map(|p| (p.name.0.clone(), p.value.0.clone()))
                .collect();
            ActionResult::Success(map)
        }

        // ── GetParameterNames response ────────────────────────────────────────
        // TODO: cwmp 0.2.4 exposes parameter_list as private in the compiled
        // artifact. Upgrade the crate or add a getter to surface the list.
        // For now we signal Done so the controller knows the call completed.
        BodyElement::GetParameterNamesResponse(_) => ActionResult::Done,

        // ── AddObject response — report the allocated instance number ─────────
        BodyElement::AddObjectResponse(r) => {
            let mut map = HashMap::new();
            map.insert("instance_number".to_string(), r.instance_number.to_string());
            map.insert("status".to_string(), r.status.0.clone());
            ActionResult::Success(map)
        }

        // ── CWMP Fault ────────────────────────────────────────────────────────
        BodyElement::Fault(f) => ActionResult::Fault {
            code: f.detail.code.to_string(),
            string: f.detail.string.0.clone(),
        },

        // ── All other responses are "done" — no result data to surface ────────
        // Covers: SetParameterValuesResponse, RebootResponse, FactoryResetResponse,
        // DeleteObjectResponse, DownloadResponse, UploadResponse, etc.
        _ => ActionResult::Done,
    };

    DeviceResponse {
        operation_id: command_id,
        device_id,
        result,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use nats_common::DeviceCommand;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn cmd(action: Action) -> DeviceCommand {
        DeviceCommand {
            command_id: Uuid::new_v4(),
            device_id: "00000-test".to_string(),
            action,
        }
    }

    #[test]
    fn get_parameter_values_produces_xml() {
        let xml = command_to_xml(&cmd(Action::GetParameterValues {
            paths: vec![
                "Device.DeviceInfo.SoftwareVersion".to_string(),
                "Device.DeviceInfo.HardwareVersion".to_string(),
            ],
        }))
        .unwrap();
        assert!(xml.contains("GetParameterValues"), "xml={xml}");
        assert!(xml.contains("SoftwareVersion"), "xml={xml}");
    }

    #[test]
    fn get_parameter_names_produces_xml() {
        let xml = command_to_xml(&cmd(Action::GetParameterNames {
            path_prefix: "Device.".to_string(),
            next_level: true,
        }))
        .unwrap();
        assert!(xml.contains("GetParameterNames"), "xml={xml}");
    }

    #[test]
    fn set_parameter_values_produces_xml() {
        let mut params = HashMap::new();
        params.insert(
            "Device.ManagementServer.PeriodicInformInterval".to_string(),
            "3600".to_string(),
        );
        let xml = command_to_xml(&cmd(Action::SetParameterValues { parameters: params })).unwrap();
        assert!(xml.contains("SetParameterValues"), "xml={xml}");
        assert!(xml.contains("PeriodicInformInterval"), "xml={xml}");
    }

    #[test]
    fn reboot_produces_xml() {
        let xml = command_to_xml(&cmd(Action::Reboot)).unwrap();
        assert!(xml.contains("Reboot"), "xml={xml}");
    }

    #[test]
    fn factory_reset_produces_xml() {
        let xml = command_to_xml(&cmd(Action::FactoryReset)).unwrap();
        assert!(xml.contains("FactoryReset"), "xml={xml}");
    }

    #[test]
    fn command_id_appears_in_xml() {
        let c = cmd(Action::Reboot);
        let id_str = c.command_id.to_string();
        let xml = command_to_xml(&c).unwrap();
        assert!(xml.contains(&id_str), "ID not found in xml={xml}");
    }
}
