use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents an abstract event originating from a device, sent by a protocol gateway (like CWMP or USP) to the core.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceEvent {
    pub device_id: String,
    pub protocol: Protocol,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub timestamp: i64,
}

/// Represents an operation sent from the core component to be executed on a device via a protocol gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceOperation {
    pub operation_id: Uuid,
    pub device_id: String,
    pub action: String,
    pub parameters: serde_json::Value,
}

/// A signal sent by the core component indicating no more operations will be sent for a given session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEnd {
    pub device_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Protocol {
    Cwmp,
    Usp,
}

// You can also add NATS connection helper functions here
// pub async fn connect_nats(url: &str) -> Result<async_nats::Client, async_nats::Error> {
//     async_nats::connect(url).await
// }
