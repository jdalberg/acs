use std::sync::Arc;

use cwmp::protocol::{BodyElement, DeviceId, HeaderElement, Inform};
use nats_common::DeviceCommand;
use redis::Client;
use tokio_stream::StreamExt;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use warp::http::{header, Response};

use crate::cwmp_translate;
use crate::nats::NatsClient;
use crate::session::{Session, SessionMap};

#[derive(Clone)]
pub(crate) struct AppState {
    pub redis: Client,
    pub nats: NatsClient,
    pub sessions: SessionMap,
}

fn build_plain_inform_response_body(id: &cwmp::protocol::ID) -> String {
    let env = cwmp::protocol::Envelope {
        cwmp_version: None,
        header: vec![HeaderElement::ID(id.clone())],
        body: vec![BodyElement::InformResponse(
            cwmp::protocol::InformResponse { max_envelopes: 1 },
        )],
    };
    cwmp::generate(&env).unwrap_or_else(|_| {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<soap-env:Envelope xmlns:soap-env="http://schemas.xmlsoap.org/soap/envelope/" xmlns:cwmp="urn:dslforum-org:cwmp-1-0">
  <soap-env:Header>
    <cwmp:ID soap-env:mustUnderstand="1"></cwmp:ID>
  </soap-env:Header>
  <soap-env:Body>
    <cwmp:InformResponse>
      <MaxEnvelopes>1</MaxEnvelopes>
    </cwmp:InformResponse>
  </soap-env:Body>
</soap-env:Envelope>"#.to_string()
    })
}

/// Called when a CPE sends an empty HTTP POST on an existing session.
///
/// An empty POST means the device has finished speaking and is asking:
/// "Do you have anything else for me?"
/// Delegates entirely to [`poll_next_command`].
async fn handle_empty_post(
    session_id: &str,
    state: Arc<AppState>,
) -> std::result::Result<Box<dyn warp::Reply>, warp::Rejection> {
    const CONTROLLER_WAIT_SECS: u64 = 5;

    let Some(session_entry) = state.sessions.get(session_id) else {
        warn!(
            session_id,
            "Empty POST for unknown session — returning empty"
        );
        return Ok(empty_xml_reply());
    };
    let session = Arc::clone(&session_entry);
    drop(session_entry);

    poll_next_command(session_id, &session, &state, CONTROLLER_WAIT_SECS).await
}

/// Called when a CPE sends a non-empty POST containing a response to a command
/// we previously issued.
///
/// Flow:
/// 1. Extract the echoed CWMP `<ID>` header — this is the `command_id` we put
///    in our original request, used to correlate the response.
/// 2. Translate the [`BodyElement`] → [`nats_common::DeviceResponse`].
/// 3. Publish the response event so the controller can process it.
/// 4. Poll for the next command (or close the session if none arrives).
async fn handle_non_inform_post(
    session_id: &str,
    state: Arc<AppState>,
    envelope: &cwmp::protocol::Envelope,
) -> Result<Box<dyn warp::Reply>, warp::Rejection> {
    const CONTROLLER_WAIT_SECS: u64 = 10;

    let Some(session_entry) = state.sessions.get(session_id) else {
        debug!(session_id, "Session not found — closing towards CPE");
        return Ok(empty_xml_reply());
    };
    let session = Arc::clone(&session_entry);
    drop(session_entry);

    let device_id = {
        let s = session.lock().await;
        s.device_id.clone()
    };

    // ── 1. Extract the echoed command_id from the CWMP ID header ──────────────
    let command_id: Option<uuid::Uuid> = envelope
        .header
        .iter()
        .find_map(|h| match h {
            HeaderElement::ID(id) => id.id.0.parse().ok(),
            _ => None,
        });

    // ── 2. Translate BodyElement → DeviceResponse ─────────────────────────────
    let device_id_str = format!("{}-{}", device_id.oui.0, device_id.serial_number.0);

    if let Some(body_element) = envelope.body.first() {
        let response =
            cwmp_translate::body_element_to_response(body_element, command_id, device_id_str);

        // ── 3. Publish to NATS ────────────────────────────────────────────────
        let payload = serde_json::to_string(&response).unwrap_or_default();
        if let Err(e) = state
            .nats
            .publish_event(
                &device_id.oui.0,
                &device_id.serial_number.0,
                "command_response",
                payload,
            )
            .await
        {
            error!(session_id, ?e, "Failed to publish command_response event");
        }
    } else {
        warn!(session_id, "Non-inform POST had empty body — skipping publish");
    }

    // ── 4. Poll for the next command ──────────────────────────────────────────
    poll_next_command(session_id, &session, &state, CONTROLLER_WAIT_SECS).await
}

/// Wait for the controller to push the next [`DeviceCommand`] for this session,
/// translate it to CWMP XML and return it to the device.
///
/// If no command arrives within `wait_secs`, publish `session_ended` and return
/// an empty 200 (which signals the CPE to disconnect).
async fn poll_next_command(
    session_id: &str,
    session: &Arc<tokio::sync::Mutex<crate::session::Session>>,
    state: &Arc<AppState>,
    wait_secs: u64,
) -> Result<Box<dyn warp::Reply>, warp::Rejection> {
    // Take subscriber out of session to avoid holding Mutex across .await
    let mut sub = {
        let mut s = session.lock().await;
        match s.command_sub.take() {
            Some(sub) => sub,
            None => {
                warn!(session_id, "No command subscriber — closing session");
                state.sessions.remove(session_id);
                return Ok(empty_xml_reply());
            }
        }
    };

    let wait =
        tokio::time::timeout(std::time::Duration::from_secs(wait_secs), sub.next()).await;

    match wait {
        // Controller sent a command — deserialise, translate, forward to device.
        Ok(Some(msg)) => {
            debug!(
                session_id,
                "Controller command received — translating to CWMP"
            );
            session.lock().await.command_sub = Some(sub);

            let cmd: DeviceCommand = match serde_json::from_slice(&msg.payload) {
                Ok(c) => c,
                Err(e) => {
                    error!(
                        session_id,
                        ?e,
                        "Malformed DeviceCommand payload — closing session"
                    );
                    publish_session_ended(session_id, session, state).await;
                    state.sessions.remove(session_id);
                    return Ok(empty_xml_reply());
                }
            };

            let xml = match cwmp_translate::command_to_xml(&cmd) {
                Ok(x) => x,
                Err(e) => {
                    error!(session_id, ?e, "Failed to translate command to CWMP XML");
                    publish_session_ended(session_id, session, state).await;
                    state.sessions.remove(session_id);
                    return Ok(empty_xml_reply());
                }
            };

            let resp = Response::builder()
                .header("Content-Type", "text/xml; charset=utf-8")
                .body(bytes::Bytes::from(xml))
                .unwrap();
            Ok(Box::new(resp))
        }

        // Subscriber closed unexpectedly.
        Ok(None) => {
            warn!(session_id, "Command subscriber stream ended unexpectedly");
            session.lock().await.command_sub = Some(sub);
            publish_session_ended(session_id, session, state).await;
            state.sessions.remove(session_id);
            Ok(empty_xml_reply())
        }

        // Timeout — controller has nothing more to say.
        Err(_elapsed) => {
            info!(
                session_id,
                wait_secs, "Controller idle — closing session"
            );
            session.lock().await.command_sub = Some(sub);
            publish_session_ended(session_id, session, state).await;
            state.sessions.remove(session_id);
            Ok(empty_xml_reply())
        }
    }
}

/// Publish a `session_ended` lifecycle event so the upstream controller knows
/// the CPE has disconnected and can free any associated resources.
async fn publish_session_ended(
    session_id: &str,
    session: &Arc<tokio::sync::Mutex<crate::session::Session>>,
    state: &Arc<AppState>,
) {
    let device_id = {
        let s = session.lock().await;
        s.device_id.clone()
    };

    let payload = serde_json::json!({
        "session_id": session_id,
        "device_id": format!("{}-{}", device_id.oui.0, device_id.serial_number.0),
        "reason": "idle_timeout",
    })
    .to_string();

    if let Err(e) = state
        .nats
        .publish_event(
            &device_id.oui.0,
            &device_id.serial_number.0,
            "session_ended",
            payload,
        )
        .await
    {
        error!(session_id, ?e, "Failed to publish session_ended event");
    }
}

/// Build a minimal empty XML reply (HTTP 200, empty body).
///
/// This is what the CPE sees when we have nothing more to send — it signals
/// "session complete, you may disconnect".
#[inline]
fn empty_xml_reply() -> Box<dyn warp::Reply> {
    Box::new(
        Response::builder()
            .status(200)
            .header("Content-Type", "text/xml; charset=utf-8")
            .body(bytes::Bytes::new())
            .unwrap(),
    )
}

/// Called when a CPE opens a new CWMP session with an Inform message.
///
/// Flow:
/// 1. Allocate a fresh `session_id` UUID.
/// 2. Subscribe to the NATS command subject for this session *before* we store
///    the session, so no command can arrive before we're ready to receive it.
///    If the subscribe fails we have no session to clean up — return 503.
/// 3. Store the new [`Session`] in the in-memory map.
/// 4. Publish the `inform` lifecycle event so the controller can react.
///    A publish failure is logged but non-fatal: the CPE is still in session and
///    the controller can recover via the next event.
/// 5. Reply with `InformResponse` and set the `session` cookie so subsequent
///    POSTs are routed to this session.
///
/// Note: the CPE will immediately follow up with an empty POST — that is handled
/// by [`handle_empty_post`] which calls [`poll_next_command`].
async fn handle_inform_post(
    state: Arc<AppState>,
    inform: &cwmp::protocol::Inform,
    header_element: Option<&HeaderElement>,
) -> Result<Box<dyn warp::Reply>, warp::Rejection> {
    let session_id = Uuid::new_v4().to_string();
    let device_id = inform.device_id.clone();

    // ── 1. Subscribe to commands *before* storing the session ─────────────────
    // This ensures no command can be lost in the window between session creation
    // and subscription setup.
    let command_sub = match state.nats.subscribe_commands(&session_id).await {
        Ok(sub) => sub,
        Err(e) => {
            error!(session_id, ?e, "Failed to subscribe to NATS command subject");
            // No session has been stored yet, so nothing to clean up.
            return Ok(Box::new(warp::reply::with_status(
                "Service unavailable",
                warp::http::StatusCode::SERVICE_UNAVAILABLE,
            )));
        }
    };

    // ── 2. Store the session ──────────────────────────────────────────────────
    let session_state = Session::new(session_id.clone(), device_id.clone(), command_sub);
    state.sessions.insert(
        session_id.clone(),
        Arc::new(tokio::sync::Mutex::new(session_state)),
    );
    debug!(session_id, oui = %device_id.oui.0, serial = %device_id.serial_number.0, "New session created");

    // ── 3. Publish `inform` lifecycle event ───────────────────────────────────
    // cwmp types don't implement serde::Serialize, so we build the JSON manually.
    let payload = serde_json::json!({
        "session_id": &session_id,
        "device_id": format!("{}-{}", &device_id.oui.0, &device_id.serial_number.0),
        "oui": &device_id.oui.0,
        "serial_number": &device_id.serial_number.0,
        "manufacturer": &device_id.manufacturer.0,
        "product_class": &device_id.product_class.0,
        "events": inform.event.iter()
            .map(|e| format!("{} {}", e.event_code.0, e.command_key.0))
            .collect::<Vec<_>>(),
        "parameter_list": inform.parameter_list.iter()
            .map(|p| (p.name.0.clone(), p.value.0.clone()))
            .collect::<std::collections::HashMap<_, _>>(),
    })
    .to_string();

    if let Err(e) = state
        .nats
        .publish_event(
            &device_id.oui.0,
            &device_id.serial_number.0,
            "inform",
            payload,
        )
        .await
    {
        // Non-fatal: the session is live; the CPE will continue the exchange.
        // The controller can detect the missing inform via a heartbeat timeout.
        error!(session_id, ?e, "Failed to publish inform event to NATS");
    }

    // ── 4. Build InformResponse and set session cookie ────────────────────────
    let id = header_element
        .and_then(|header| match header {
            HeaderElement::ID(id) => Some(id.clone()),
            _ => None,
        })
        .unwrap_or_else(|| cwmp::protocol::ID::new(false, ""));

    let body = build_plain_inform_response_body(&id);
    let mut response = Response::builder()
        .status(warp::http::StatusCode::OK)
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(bytes::Bytes::from(body))
        .unwrap();

    let cookie_header = format!("session={session_id}; HttpOnly; Path=/; Max-Age=3600");
    response
        .headers_mut()
        .append(header::SET_COOKIE, cookie_header.parse().unwrap());

    Ok(Box::new(response))
}


pub(crate) async fn handle_cwmp_request(
    cookie: Option<String>,
    body: bytes::Bytes,
    state: std::sync::Arc<AppState>,
) -> Result<Box<dyn warp::Reply>, warp::Rejection> {
    info!("Received CWMP request. Cookie: {:?}", cookie);

    if let Some(session_cookie) = cookie {
        info!("Session Cookie: {:?}", session_cookie);

        if body.is_empty() {
            info!("Empty POST on existing session — sending next command");
            return handle_empty_post(&session_cookie, state).await;
        } else {
            // Map parse error to String immediately so the Result is Send
            // across the .await boundary inside handle_non_inform_post.
            match cwmp::parse_bytes(body.as_ref()).map_err(|e| e.to_string()) {
                Ok(parsed_envelope) => {
                    info!("Command response: {:?}", parsed_envelope);
                    return handle_non_inform_post(
                        &session_cookie,
                        state.clone(),
                        &parsed_envelope,
                    )
                    .await;
                }
                Err(e) => {
                    error!("Error parsing command response: {e}");
                    return Ok(Box::new(warp::reply::with_status(
                        "Error parsing XML",
                        warp::http::StatusCode::BAD_REQUEST,
                    )));
                }
            }
        }
    } else {
        if body.is_empty() {
            error!("Empty body with no session cookie — rejecting");
            return Ok(Box::new(warp::reply::with_status(
                "Bad Request",
                warp::http::StatusCode::BAD_REQUEST,
            )));
        }

        match cwmp::parse_bytes(body.as_ref()).map_err(|e| e.to_string()) {
            Ok(parsed_envelope) => {
                let device_id: Option<DeviceId> = parsed_envelope
                    .body
                    .iter()
                    .filter_map(|v| match v {
                        BodyElement::Inform(inform) => Some(inform.device_id.clone()),
                        _ => None,
                    })
                    .next();

                if let Some(device_id) = device_id {
                    info!("Inform from device: {:?}", device_id);
                    let inform: Inform = parsed_envelope
                        .body
                        .iter()
                        .filter_map(|v| match v {
                            BodyElement::Inform(inform) => Some(inform.clone()),
                            _ => None,
                        })
                        .next()
                        .unwrap();
                    let header_element = parsed_envelope
                        .header
                        .iter()
                        .find(|h| matches!(h, HeaderElement::ID(_)));
                    return handle_inform_post(state.clone(), &inform, header_element).await;
                } else {
                    error!("Non-Inform body received without a session cookie — rejecting");
                    return Ok(Box::new(warp::reply::with_status(
                        "Bad Request",
                        warp::http::StatusCode::BAD_REQUEST,
                    )));
                }
            }
            Err(e) => {
                error!("Error parsing Inform: {e}");
                return Ok(Box::new(warp::reply::with_status(
                    "Error parsing XML",
                    warp::http::StatusCode::BAD_REQUEST,
                )));
            }
        }
    }
}
