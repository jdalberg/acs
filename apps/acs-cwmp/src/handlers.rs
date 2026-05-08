use cwmp::protocol::{BodyElement, DeviceId};
use redis::Client;
use tracing::{error, info};

#[derive(Clone)]
pub(crate) struct AppState {
    pub redis: Client,
    // nats: NatsClient,
}

/// This is the main handler for the Inform route.
/// It will parse the incoming request, validate it, and then
/// start a CWMP session with the device.
///
/// This should be an Inform message, and the session should be started
/// by generating a cookie, store it in the `DashMap` and include it in the response.
///
/// HTTP wise, we will set the Connection: keep-alive header, unless the cpe specifically sends the
/// Connection: close header. This will make sure that the connection is reused for the next request
/// from the CPE.
///
/// START
///  │
///  ├─► Device sends: Inform
///  │       └─ ACS replies: `InformResponse`
///  │
///  ├─► Device sends: Empty HTTP POST
///  │       └─ ACS replies: First Command (e.g., `GetParameterNames`)
///  │
///  ├─► Device sends: Command Response
///  │       └─ ACS replies: Next Command (or Empty HTTP Response to end session)
///  │
///  └─► Repeat until ACS sends Empty Response → Session Ends
///
/// So in the handler we have 4 cases:
///
/// 1. Inform message: Start a session, generate a cookie, store it in the `DashMap`
/// 2. Empty HTTP POST: Fire of the next pending command from the queue or terminate the session
/// 3. Command Response: Send upstream and fire of the next pending command from the queue or terminate the session
/// 4. Any other message: Terminate the session
pub(crate) async fn handle_cwmp_request(
    cookie: Option<String>,
    body: bytes::Bytes,
    state: std::sync::Arc<AppState>,
) -> Result<impl warp::Reply, warp::Rejection> {
    info!("Received CWMP request. Cookie: {:?}", cookie);
    // TODO: Manage session state via state.redis
    // TODO: Dispatch abstract events via state.nats

    // Parse the incoming XML payload
    match cwmp::parse_bytes(body.as_ref()) {
        Ok(parsed_envelope) => {
            // handle the parsed envelope if it is an inform
            // Informs are special, because they initiate a session, and don't need a cookie.
            // We need to get the device_id from the inform

            // If there is a cookie, the session should be ongoing, and we should be
            // able to load it from the source ip alone.    ( we will add source ip to the cookie )
            // If there is no cookie, we need to find the device id in the body.
            // This will be the case for "Initiate-Assisted-Bonding" "Initiate-Bonding" and "Inform".

            if let Some(session_cookie) = cookie {
                info!("Session Cookie: {:?}", session_cookie);
            } else {
                let device_id: Option<DeviceId> = parsed_envelope
                    .body
                    .iter()
                    .filter_map(|v| match v {
                        BodyElement::Inform(inform) => Some(inform.device_id.clone()),
                        _ => None,
                    })
                    .next();

                if let Some(device_id) = device_id {
                    info!("Device ID: {:?}", device_id);
                } else {
                    error!("No device ID found in parsed envelope");
                }
            }

            // Send the event upstream
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
