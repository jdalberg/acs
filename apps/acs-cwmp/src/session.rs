use std::sync::Arc;

use async_nats::Subscriber;
use cwmp::protocol::DeviceId;
use dashmap::DashMap;
use tokio::sync::Mutex;

/// Represents one active CWMP session with a CPE device.
///
/// The session is created when an Inform is received and removed when
/// the session ends (timeout, CPE disconnect, or empty response sent).
///
/// Dropping this struct automatically unsubscribes from the NATS command
/// subject, since `Subscriber` cleans up on drop.
pub struct Session {
    /// Unique session identifier, used as the NATS routing key:
    /// `acs.sessions.{session_id}.command`
    pub session_id: String,

    /// The device that owns this session.
    pub device_id: DeviceId,

    /// NATS subscriber for commands directed at this session.
    /// Taken out before awaiting so we don't hold the Mutex across an await.
    /// Replaced after the await completes.
    pub command_sub: Option<Subscriber>,
}

impl Session {
    pub fn new(session_id: String, device_id: DeviceId, command_sub: Subscriber) -> Self {
        Self {
            session_id,
            device_id,
            command_sub: Some(command_sub),
        }
    }
}

/// Thread-safe, in-process store of all active sessions on this pod.
///
/// Sessions are keyed by `session_id` (= the cookie value sent to the CPE).
/// When a session is removed, its `Session` is dropped and the NATS
/// subscription is automatically cancelled.
pub type SessionMap = Arc<DashMap<String, Arc<Mutex<Session>>>>;

/// Construct a new empty session map.
pub fn new_session_map() -> SessionMap {
    Arc::new(DashMap::new())
}
