use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The protocol gateway that handled the device
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Protocol {
    Cwmp,
    Usp,
}

/// A normalized event from the device (Abstracts CWMP Inform and USP Notify)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceEvent {
    /// Unique identifier for the device (e.g., OUI-SerialNumber)
    pub device_id: String,
    
    /// Which protocol gateway received this
    pub protocol: Protocol,

    /// Reasons the device is contacting the ACS (Boot, Periodic, ValueChange, etc.)
    pub triggers: Vec<EventTrigger>,

    /// A map of TR-181 (or TR-098) parameters the device included in its message.
    /// In CWMP this is the ParameterList in the Inform.
    pub parameters: HashMap<String, String>,

    /// Unix timestamp when the gateway received the message
    pub timestamp: i64,
}

/// Abstract triggers representing *why* the device is communicating
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventTrigger {
    Boot,
    Periodic,
    ValueChange(String), // The parameter path that changed
    DiagnosticsComplete,
    ConnectionRequest,
    TransferComplete,
    Custom(String), // Fallback for vendor-specific events (e.g. "X_VENDOR_Event")
}

/// A command the controller wants to execute on a device.
///
/// This type is protocol-agnostic — each protocol pod (acs-cwmp, acs-usp, …)
/// deserialises it from NATS and translates it into the appropriate wire format.
///
/// # Correlation
///
/// `command_id` is forwarded verbatim into the protocol's native request ID
/// field (e.g. CWMP `<ID>` header). The CPE echoes the ID back in its response,
/// allowing the pod to route the response back to the controller without
/// maintaining a separate ID map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCommand {
    /// Unique identifier for this command instance.
    /// Used as the CWMP `<ID>` header / USP request correlation token.
    pub command_id: Uuid,

    /// The device this command targets (e.g. "AABBCC-1234567").
    pub device_id: String,

    /// What the controller wants the device to do.
    pub action: Action,
}

/// The specific actions the controller can ask a device to perform.
///
/// Variants are protocol-agnostic. Protocol pods translate them to wire format.
/// Variants marked *(CWMP-only)* have no direct equivalent in TR-369/USP; USP
/// pods should return an `UnsupportedAction` error for those.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    /// Read one or more parameter values by exact path.
    GetParameterValues {
        paths: Vec<String>,
    },

    /// Discover parameters under a path prefix. *(CWMP-only)*
    GetParameterNames {
        path_prefix: String,
        /// `true` → only immediate children; `false` → full subtree.
        next_level: bool,
    },

    /// Write parameter values. Type defaults to `xsd:string`.
    SetParameterValues {
        parameters: HashMap<String, String>,
    },

    /// Create a new object instance under a multi-instance object path.
    AddObject {
        path: String,
    },

    /// Delete an existing object instance.
    DeleteObject {
        path: String,
    },

    /// Reboot the device.
    Reboot,

    /// Restore factory defaults.
    FactoryReset,

    /// Ask the device to download a file (firmware, config, …).
    Download {
        url: String,
        /// e.g. `"1 Firmware Upgrade Image"`, `"3 Vendor Configuration File"`
        file_type: String,
        /// Expected file size in bytes (0 = unknown).
        file_size: u32,
        /// Destination filename on the device (empty = device chooses).
        target_filename: String,
    },

    /// Ask the device to upload a file (log, config backup, …).
    Upload {
        url: String,
        /// e.g. `"1 Vendor Configuration File"`
        file_type: String,
    },
}

/// A signal sent by the core component indicating no more operations will be sent for a given session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEnd {
    pub device_id: String,
    pub reason: String,
}

/// A normalized response from the device after executing a DeviceOperation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceResponse {
    pub operation_id: Option<Uuid>, // Optional for now since we might not have it in all flows
    pub device_id: String,
    pub result: ActionResult,
}

/// The specific results from executing an Action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionResult {
    Success(HashMap<String, String>), // Used for GetParameterValues
    Fault { code: String, string: String }, // Used for any CWMP fault
    Done, // Generic success for actions with no return value
}

/// A signal sent by the protocol handler to notify the core of a session's lifecycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionLifecycle {
    Started { session_id: String, device_id: String },
    Ended { session_id: String, device_id: String, reason: String },
}
