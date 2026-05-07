use clap::Parser;
use redis::Client as RedisClient;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use warp::Filter;

mod session;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Config {
    /// The URL of the Redis server
    #[arg(long, env = "REDIS_URL", default_value = "redis://127.0.0.1/")]
    pub redis_url: String,

    /// The URL of the NATS server
    #[arg(long, env = "NATS_URL", default_value = "nats://127.0.0.1:4222")]
    pub nats_url: String,

    /// The port to run the web server on
    #[arg(short, long, env = "PORT", default_value_t = 8080)]
    pub port: u16,
}
// use async_nats::Client as NatsClient;

#[derive(Clone)]
struct AppState {
    redis: RedisClient,
    // nats: NatsClient,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize JSON logging for Elasticsearch
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer().json())
        .init();
    // Parse configuration from command line and environment variables
    let config = Config::parse();

    info!("Starting acs-cwmp service on port {}...", config.port);

    // 1. Initialize Redis for Session State
    info!("Connecting to Redis at {}", config.redis_url);
    let redis_client = RedisClient::open(config.redis_url.clone())?;

    // 2. Initialize NATS for IPC
    // info!("Connecting to NATS at {}", config.nats_url);
    // let nats_client = async_nats::connect(&config.nats_url).await?;

    // App State to share across routes
    let state = Arc::new(AppState {
        redis: redis_client,
        // nats: nats_client,
    });

    // Extract state filter for Warp
    let state_filter = warp::any().map(move || state.clone());

    // 3. Define Warp Routes
    // A simple health check route
    let health_route = warp::path!("health")
        .and(warp::get())
        .map(|| warp::reply::json(&"OK"));

    // The main CWMP endpoint
    let cwmp_route = warp::path!("cwmp")
        .and(warp::post())
        .and(warp::header::optional::<String>("cookie"))
        .and(warp::body::bytes())
        .and(state_filter.clone())
        .and_then(handle_cwmp_request);

    let routes = health_route.or(cwmp_route);

    // 4. Start HTTP Server
    info!("Listening on http://0.0.0.0:{}", config.port);
    warp::serve(routes).run(([0, 0, 0, 0], config.port)).await;

    Ok(())
}

async fn handle_cwmp_request(
    cookie: Option<String>,
    body: bytes::Bytes,
    state: Arc<AppState>,
) -> Result<impl warp::Reply, warp::Rejection> {
    info!("Received CWMP request. Cookie: {:?}", cookie);
    // TODO: Parse the CWMP XML body
    // TODO: Manage session state via state.redis
    // TODO: Dispatch abstract events via state.nats
    // Parse the incoming XML payload
    match cwmp::parse_bytes(body.as_ref()) {
        Ok(parsed_envelope) => {
            // handle the parsed envelope if it is an inform
            if parsed_envelope.is_inform() {
                // Send the event upstream
            }
        }
        Err(e) => {
            error!("Error parsing XML: {:?}", e);
            return Ok(warp::reply::with_status(
                "Error parsing XML",
                warp::http::StatusCode::BAD_REQUEST,
            ));
        }
    }
    Ok(warp::reply::with_status(
        "CWMP Handler Stub",
        warp::http::StatusCode::OK,
    ))
}
