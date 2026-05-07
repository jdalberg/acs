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

/// A normalized operation the Core wants to execute on the device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceOperation {
    pub operation_id: Uuid,
    pub device_id: String,
    pub action: Action,
}

/// The specific actions the core can ask the device to perform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    SetParameterValues {
        parameters: HashMap<String, String>,
    },
    GetParameterValues {
        paths: Vec<String>,
    },
    AddObject {
        path: String,
    },
    DeleteObject {
        path: String,
    },
    Reboot,
    FactoryReset,
    Download {
        url: String,
        file_type: String, // e.g. "1 Firmware Upgrade Image"
    },
}

/// A signal sent by the core component indicating no more operations will be sent for a given session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEnd {
    pub device_id: String,
    pub reason: String,
}
