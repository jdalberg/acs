//! NATS integration for the acs-cwmp pod.
//!
//! ## Subject layout
//!
//! Pod → Controller (Events, JetStream stream `ACS_EVENTS`):
//!   `acs.events.{oui}.{serial}.inform`
//!   `acs.events.{oui}.{serial}.command_response`
//!   `acs.events.{oui}.{serial}.session_ended`
//!
//! Controller → Pod (Commands, core NATS — no persistence needed):
//!   `acs.sessions.{session_id}.command`

use async_nats::{Client, Subscriber};
use bytes::Bytes;

/// Thin wrapper around the async-nats client providing the two
/// operations the CWMP pod needs.
#[derive(Clone)]
pub struct NatsClient {
    inner: Client,
}

impl NatsClient {
    pub fn new(inner: Client) -> Self {
        Self { inner }
    }

    /// Publish an event to the controller on the JetStream event stream.
    ///
    /// `event_type` should be one of: `inform`, `command_response`, `session_ended`.
    pub async fn publish_event(
        &self,
        oui: &str,
        serial: &str,
        event_type: &str,
        payload: impl Into<Bytes>,
    ) -> Result<(), async_nats::PublishError> {
        let subject = format!("acs.events.{oui}.{serial}.{event_type}");
        self.inner.publish(subject, payload.into()).await
    }

    /// Subscribe to commands directed at a specific session.
    ///
    /// Returns a `Subscriber` that yields one message per command.
    /// Dropping the subscriber automatically unsubscribes.
    pub async fn subscribe_commands(
        &self,
        session_id: &str,
    ) -> Result<Subscriber, async_nats::SubscribeError> {
        let subject = format!("acs.sessions.{session_id}.command");
        self.inner.subscribe(subject).await
    }
}
