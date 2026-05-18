use std::sync::Arc;

use dashmap::DashMap;
use nats_common::DeviceResponse;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::nats::NatsClient;

#[derive(Clone)]
pub struct ApiState {
    pub pool: sqlx::PgPool,
    pub nats: NatsClient,
    /// Maps `device_uid` -> `session_id`
    pub active_sessions: Arc<DashMap<String, String>>,
    /// Maps `command_id` -> oneshot sender for waiting on command response
    pub pending_commands: Arc<DashMap<Uuid, oneshot::Sender<DeviceResponse>>>,
}

impl ApiState {
    pub fn new(pool: sqlx::PgPool, nats: NatsClient) -> Self {
        Self {
            pool,
            nats,
            active_sessions: Arc::new(DashMap::new()),
            pending_commands: Arc::new(DashMap::new()),
        }
    }
}
