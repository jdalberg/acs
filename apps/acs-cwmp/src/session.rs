use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CwmpSession {
    pub device_id: String,
    pub current_state: String,
    // Add other state data like pending operations
}

#[derive(Clone)]
pub struct SessionStore {
    client: redis::Client,
}

impl SessionStore {
    pub fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        Ok(Self { client })
    }

    /// Save a session to Redis with a TTL (e.g., 60 seconds)
    pub async fn save_session(
        &self,
        session_id: &str,
        session: &CwmpSession,
    ) -> Result<(), redis::RedisError> {
        let mut con = self.client.get_async_connection().await?;
        let serialized = serde_json::to_string(session).unwrap();

        // SETEX: Set Key, TTL, and Value atomically
        con.set_ex(format!("session:{}", session_id), serialized, 60)
            .await?;
        Ok(())
    }

    /// Retrieve a session from Redis
    pub async fn get_session(
        &self,
        session_id: &str,
    ) -> Result<Option<CwmpSession>, redis::RedisError> {
        let mut con = self.client.get_async_connection().await?;
        let value: Option<String> = con.get(format!("session:{}", session_id)).await?;

        if let Some(json) = value {
            let session: CwmpSession = serde_json::from_str(&json).unwrap();
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }
}
