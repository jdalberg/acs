use cwmp::protocol::BodyElement;
use serde::Serialize;
use uuid::Uuid;

use crate::errors::AcsError;

#[derive(Debug, Serialize, Default)]
pub enum EventType {
    Inform,
    #[default]
    Unknown,
}

#[derive(Debug, Serialize, Default)]
pub enum SessionState {
    Init,
    #[default]
    Unknown,
}

#[derive(Debug, Serialize)]
pub struct Event {
    event_code: String,
    command_key: String,
}

impl From<&cwmp::protocol::EventStruct> for Event {
    fn from(event_struct: &cwmp::protocol::EventStruct) -> Self {
        Event {
            event_code: event_struct.event_code.0.clone(),
            command_key: event_struct.command_key.0.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct EventParameter {
    name: String,
    value: String,
}

impl From<&cwmp::protocol::ParameterValue> for EventParameter {
    fn from(param_value: &cwmp::protocol::ParameterValue) -> Self {
        EventParameter {
            name: param_value.name.0.clone(),
            value: param_value.value.0.clone(),
        }
    }
}

#[derive(Debug, Serialize, Default)]
pub struct EventMessage {
    acs_instance_id: String,
    session_id: String,
    session_type: EventType,
    session_state: SessionState,
    pub device_id: String,
    pub events: Option<Vec<Event>>,
    pub parameters: Option<Vec<EventParameter>>,
}

fn find_inform(envelope: &cwmp::protocol::Envelope) -> Option<&cwmp::protocol::Inform> {
    let informs: Vec<&BodyElement> = envelope
        .body
        .iter()
        .filter(|v| matches!(v, BodyElement::Inform(_)))
        .collect();

    match informs.first() {
        Some(&BodyElement::Inform(inform)) => Some(inform),
        _ => None,
    }
}

fn find_events(inform: &cwmp::protocol::Inform) -> Vec<Event> {
    inform.event.iter().map(|v| Event::from(v)).collect()
}

fn find_parameters(inform: &cwmp::protocol::Inform) -> Vec<EventParameter> {
    inform
        .parameter_list
        .iter()
        .map(|v| EventParameter::from(v))
        .collect()
}

fn construct_device_id(inform: &cwmp::protocol::Inform) -> String {
    format!(
        "{}-{}-{}-{}",
        inform.device_id.manufacturer.0,
        inform.device_id.oui.0,
        inform.device_id.serial_number.0,
        inform.device_id.product_class.0,
    )
}

// Implement the conversion from a cwmp::protocol::Envelope to an EventMessage
impl TryFrom<&cwmp::protocol::Envelope> for EventMessage {
    type Error = AcsError;

    fn try_from(inform_envelope: &cwmp::protocol::Envelope) -> Result<Self, Self::Error> {
        let pod_name = std::env::var("POD_NAME").unwrap_or_else(|_| "unknown_pod".to_string());

        // Create a unique session id based on valued in the inform envelope
        if let Some(inform) = find_inform(inform_envelope) {
            // TODO: Construct a better session id - come up with a concept
            let session_id = format!(
                "{}-{}-{}-{}",
                inform.device_id.manufacturer.0,
                inform.device_id.oui.0,
                inform.device_id.serial_number.0,
                inform.device_id.product_class.0,
            );
            Ok(EventMessage {
                acs_instance_id: pod_name,
                session_id,
                session_type: EventType::Inform,
                session_state: SessionState::Init,
                events: Some(find_events(inform)),
                parameters: Some(find_parameters(inform)),
                device_id: construct_device_id(inform),
            })
        } else {
            Err(AcsError::NoInformPayload)
        }
    }
}

impl EventMessage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn kafka_key(&self) -> String {
        self.session_id.clone()
    }

    pub fn as_kafka_message(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

#[derive(Debug)]
enum PolicyType {
    GetParameterValues,
    SetParameterValues,
    GetParameterNames,
}

#[derive(Debug)]
pub struct PolicyMessage {
    acs_instance_id: String,
    session_id: String,
    session_type: String,
    session_state: String,
    device_id: String,
    policy_type: PolicyType,
    policy_parameters: Vec<String>,
}

#[derive(Debug)]
pub struct PolicyMessageResponse {
    acs_instance_id: String,
    session_id: String,
    session_type: String,
    session_state: String,
    device_id: String,
    policy_type: PolicyType,
    response: String, // json version of the response from the device
}
