//! NATS integration for the acs-controller.
//!
//! ## Subject layout (controller perspective)
//!
//! Protocol pods → Controller (subscribed here):
//!   `acs.events.{oui}.{serial}.inform`
//!   `acs.events.{oui}.{serial}.command_response`
//!   `acs.events.{oui}.{serial}.session_ended`
//!
//! Controller → Protocol pods (published elsewhere, not yet implemented):
//!   `acs.sessions.{session_id}.command`

use async_nats::{Client, Subscriber};

/// Thin wrapper around the async-nats client for the controller.
#[derive(Clone)]
pub struct NatsClient {
    inner: Client,
}

impl NatsClient {
    pub fn new(inner: Client) -> Self {
        Self { inner }
    }

    /// Subscribe to **all** device events from every protocol pod.
    ///
    /// Uses the NATS `>` wildcard which matches any number of subject tokens,
    /// so this single subscription receives every event regardless of device
    /// OUI, serial, or event type.
    ///
    /// Extract the event type from the last dot-separated token of each
    /// message's subject:
    /// ```text
    /// "acs.events.AABBCC.1234567.inform" → event_type = "inform"
    /// ```
    pub async fn subscribe_events(&self) -> Result<Subscriber, async_nats::SubscribeError> {
        self.inner.subscribe("acs.events.>").await
    }
}
