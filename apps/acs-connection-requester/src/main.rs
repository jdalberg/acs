use anyhow::{Context, Result};
use clap::Parser;
use tokio_stream::StreamExt;
use serde::Deserialize;
use tracing::{error, info, debug};

#[derive(Parser, Debug)]
#[command(author, version, about = "ACS Connection Requester Service")]
struct Args {
    #[arg(long, env = "NATS_URL", default_value = "nats://localhost:4222")]
    nats_url: String,
}

#[derive(Debug, Deserialize)]
struct ConnectionRequestPayload {
    device_id: String,
    connection_request_url: String,
    username: Option<String>,
    password: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    info!("Connecting to NATS at {}", args.nats_url);
    let client = async_nats::connect(&args.nats_url)
        .await
        .context("Failed to connect to NATS")?;

    let subject = "acs.connection.request";
    info!("Subscribing to NATS subject: {}", subject);
    let mut subscriber = client.subscribe(subject).await.context("Failed to subscribe")?;

    let http_client = reqwest::Client::new();

    while let Some(message) = subscriber.next().await {
        let payload: ConnectionRequestPayload = match serde_json::from_slice(&message.payload) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to parse connection request payload: {}", e);
                continue;
            }
        };

        info!(
            device_id = %payload.device_id,
            url = %payload.connection_request_url,
            "Received connection request"
        );

        // Perform the HTTP GET
        let mut req = http_client.get(&payload.connection_request_url);
        
        // Add basic auth if provided
        if let (Some(username), Some(password)) = (payload.username, payload.password) {
            req = req.basic_auth(username, Some(password));
        }

        tokio::spawn(async move {
            match req.send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        debug!(device_id = %payload.device_id, status = %response.status(), "Connection request successful");
                    } else {
                        error!(device_id = %payload.device_id, status = %response.status(), "Connection request failed with status");
                    }
                }
                Err(e) => {
                    error!(device_id = %payload.device_id, ?e, "HTTP request failed");
                }
            }
        });
    }

    Ok(())
}
