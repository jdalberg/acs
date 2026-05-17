//! acs-controller — main entrypoint.
//!
//! Connects to NATS and PostgreSQL, then runs the event loop that dispatches
//! device events from all protocol pods to the appropriate handler.

use clap::Parser;
use sqlx::postgres::PgPoolOptions;
use tokio_stream::StreamExt;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

mod db;
mod handlers;
mod nats;

// ── Configuration ─────────────────────────────────────────────────────────────

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Config {
    /// NATS server URL.
    #[arg(long, env = "NATS_URL", default_value = "nats://127.0.0.1:4222")]
    pub nats_url: String,

    /// PostgreSQL connection URL.
    /// Example: `postgres://user:pass@localhost:5432/acs`
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: String,

    /// UUID of the domain to assign newly-seen devices to.
    ///
    /// Bootstrap procedure:
    ///   1. Apply `db/domains.sql` to your Postgres instance.
    ///   2. `INSERT INTO domains (name, slug) VALUES ('Default', 'default');`
    ///   3. `SELECT id FROM domains WHERE slug = 'default';`
    ///   4. Set this env var (or `--default-domain-id`) to that UUID.
    #[arg(long, env = "DEFAULT_DOMAIN_ID")]
    pub default_domain_id: Uuid,

    /// Maximum number of PostgreSQL connections to keep open.
    #[arg(long, env = "DB_MAX_CONNECTIONS", default_value_t = 5)]
    pub db_max_connections: u32,
}

// ── Entrypoint ────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    let config = Config::parse();

    // ── NATS ──────────────────────────────────────────────────────────────────
    info!(nats_url = %config.nats_url, "Connecting to NATS");
    let nats_inner = async_nats::connect(&config.nats_url).await?;
    let nats = nats::NatsClient::new(nats_inner);

    // ── PostgreSQL ────────────────────────────────────────────────────────────
    info!("Connecting to PostgreSQL");
    let pool = PgPoolOptions::new()
        .max_connections(config.db_max_connections)
        .connect(&config.database_url)
        .await?;

    info!(
        default_domain_id = %config.default_domain_id,
        "acs-controller ready — entering event loop",
    );

    event_loop(nats, pool, config.default_domain_id).await;

    Ok(())
}

// ── Event loop ────────────────────────────────────────────────────────────────

/// Receive and dispatch all device events from all protocol pods.
///
/// Loops forever until the NATS connection drops. Event type is derived from
/// the last token of the NATS subject so no separate metadata field is needed.
async fn event_loop(nats: nats::NatsClient, pool: sqlx::PgPool, default_domain_id: Uuid) {
    let mut subscriber = match nats.subscribe_events().await {
        Ok(s) => s,
        Err(e) => {
            error!(?e, "Failed to subscribe to acs.events.> — cannot start event loop");
            return;
        }
    };

    while let Some(msg) = subscriber.next().await {
        let subject = msg.subject.as_str();

        // Derive the event type from the last dot-separated segment.
        // "acs.events.AABBCC.1234567.inform" → "inform"
        let event_type = subject.rsplit('.').next().unwrap_or("unknown");

        match event_type {
            "inform" => {
                if let Err(e) =
                    handlers::inform::handle_inform(&msg.payload, &pool, default_domain_id).await
                {
                    error!(subject, ?e, "inform handler failed");
                }
            }

            "command_response" => {
                // Future: look up the session, correlate the response UUID,
                // publish result back to the originating API caller.
                info!(subject, "command_response received — handler not yet implemented");
            }

            "session_ended" => {
                // Future: close any pending commands for this session,
                // update last_seen in devices.
                info!(subject, "session_ended received — handler not yet implemented");
            }

            other => {
                warn!(subject, event_type = other, "Unknown event type — ignoring");
            }
        }
    }

    // The subscriber only ends if the NATS server closed the connection.
    error!("NATS event subscriber ended — acs-controller shutting down");
}
