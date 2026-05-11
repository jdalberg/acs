use cwmp::protocol::{BodyElement, DeviceId};
use redis::Client;
use tracing::{error, info};

use crate::nats::NatsClient;
use crate::session::SessionMap;

#[derive(Clone)]
pub(crate) struct AppState {
    pub redis: Client,
    pub nats: NatsClient,
    pub sessions: SessionMap,
}

// Here we have to check if there is a command available for the session
async fn handle_empty_post(
    session_id: &str,
    session_map: SessionMap,
    redis_manager: Arc<RedisManager>,
) -> std::result::Result<Box<dyn warp::Reply>, warp::Rejection> {
    // Check if the session exists
    if let Some(session_entry) = session_map.get(session_id) {
        let session = Arc::clone(&session_entry);

        // TODO: Check of this session is an API session, it that is the case
        // we should NOT wait for a command, but just send the cached command
        // and then report the result in to handle_non_inform_post
        let (source, device_id) = {
            let state = session.lock().await;
            (state.source.clone(), state.device_id.clone())
        };

        if let Source::Api = source {
            debug!("Session is an API session, not waiting for a command, but pick-and-delete from cache instead, with device_id: {device_id}");

            match redis_manager
                .get_key(&format!("acs:commands:{device_id}"))
                .await
            {
                Ok(Some(reply_to_and_command_json)) => {
                    // Send the command in the response

                    let (reply_to, command_json) = reply_to_and_command_json
                        .split_once(':')
                        .map(|(reply_to, json)| (reply_to.to_string(), json.to_string()))
                        .unwrap_or_default();
                    match serde_json::from_str::<AcsCommand>(&command_json) {
                        Ok(command) => {
                            debug!("Found command in cache: {:?}", command);
                            let new_outstanding_request_id = uuid::Uuid::new_v4().to_string();
                            let mut state = session.lock().await;
                            state.outstanding_request_id = Some(new_outstanding_request_id.clone());
                            state.reply_to = reply_to.clone();
                            let body =
                                build_cpe_response_body(&command, &new_outstanding_request_id);
                            let resp = Response::builder()
                                .header("Content-Type", "text/xml; charset=utf-8")
                                .body(body)
                                .unwrap();
                            Ok(Box::new(resp))
                        }
                        Err(e) => {
                            error!("Failed to parse command JSON: {:?}, ending session", e);
                            Ok(Box::new(warp::reply::with_header(
                                "",
                                "Content-Type",
                                "text/xml; charset=utf-8",
                            )))
                        }
                    }
                }
                Ok(None) => {
                    warn!("No command found in cache, weird. Ending session");
                    Ok(Box::new(warp::reply::with_header(
                        "",
                        "Content-Type",
                        "text/xml; charset=utf-8",
                    )))
                }
                Err(e) => {
                    warn!("Error retrieving command from cache {e}, weird. Ending session");
                    Ok(Box::new(warp::reply::with_header(
                        "",
                        "Content-Type",
                        "text/xml; charset=utf-8",
                    )))
                }
            }
        } else {
            // Step 1: Take the receiver
            let command_rx = {
                let mut state = session.lock().await;
                state.last_seen = Some(tokio::time::Instant::now());
                state.command_rx.take()
            };

            // Try to receive the next command for this session
            if let Some(mut rx) = command_rx {
                // If the command channel is closed, we can remove the session
                if rx.is_closed() {
                    session_map.remove(session_id);
                    Ok(Box::new(warp::reply::with_header(
                        "",
                        "Content-Type",
                        "text/xml; charset=utf-8",
                    )))
                } else {
                    // Step 2: Await the command
                    if let Ok(Some(command)) =
                        tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await
                    {
                        // Step 3: Put receiver back
                        let new_outstanding_request_id = uuid::Uuid::new_v4().to_string();
                        let mut state = session.lock().await;
                        state.outstanding_request_id = Some(new_outstanding_request_id.clone());
                        state.command_rx = Some(rx);

                        let body = build_cpe_response_body(&command, &new_outstanding_request_id);
                        let resp = Response::builder()
                            .header("Content-Type", "text/xml; charset=utf-8")
                            .body(body)
                            .unwrap();
                        Ok(Box::new(resp))
                    } else {
                        // Step 3b: Put receiver back even if nothing arrived
                        let mut state = session.lock().await;
                        state.command_rx = Some(rx);
                        Ok(Box::new(warp::reply::with_header(
                            "",
                            "Content-Type",
                            "text/xml; charset=utf-8",
                        )))
                    }
                }
            } else {
                // No command receiver available, session is closed
                debug!("No command receiver available, session is closed");
                session_map.remove(session_id);
                Ok(Box::new(warp::reply::with_header(
                    "",
                    "Content-Type",
                    "text/xml; charset=utf-8",
                )))
            }
        }
    } else {
        // Session not found — reject or send basic empty response
        debug!("Session not found, closing session towards CPE");
        Ok(Box::new(warp::reply::with_header(
            "",
            "Content-Type",
            "text/xml; charset=utf-8",
        )))
    }
}

// We need to return an InformResponse to the CPE
// The Inform information must also be delivered to the backend, but
// no waiting for the backend reply is needed
async fn handle_inform_post(
    nats: &NatsClient,
    cpe_addr: Option<SocketAddr>,
    session_map: SessionMap,
    inform: &cwmp::protocol::Inform,
    header_element: Option<&HeaderElement>,
    redis_manager: Arc<RedisManager>,
) -> Result<Box<dyn warp::Reply>, warp::Rejection> {
    let device_id = generate_device_id(inform);
    debug!("Parsed Inform message, generated device_id: {}", device_id);
    // TODO: This could be the result of an API generated connection request
    // in that case this inform will have the CONNECTION REQUEST event in the event set
    // and we should have the original command in the cache.
    //
    // If this is the case, we should mark the session as ongoing in the cache, so that when the empty arrives the
    // session id is marked for API and the command can be retrieved from the cache and sent to the device immediately
    // The following response should then be send to the reply-to of the original command and then the session and the cache should be cleaned up

    // When we see an Inform message, nothing should be in the cache unless this is an API inform
    // So this should return (Api, real API correlation-id) or (Cpe, None)
    // We should therefore assume Cpe,None unless this is a CONNECTION REQUEST inform
    let (source, correlation_id) = if contains_event_type(inform, "6 CONNECTION REQUEST") {
        debug!("Connection request event found in inform message, could be API inform");
        find_source(redis_manager, &inform.device_id.serial_number.0).await
    } else {
        debug!("No connection request event found in inform message");
        (Source::Cpe, None)
    };
    debug!(
        "Found source: {:?}, correlation-id: {:?}",
        source, correlation_id
    );

    // In this case a cookie should not be present in the request. If it is present
    // we should remove it from the DashMap and generate a new one for this session
    if let Some(session_cookie_value) = session_cookie {
        warn!(
            "Inform seen with session cookie already set, removing old session: {}",
            session_cookie_value
        );
        session_map.remove(&session_cookie_value);
    }
    let session_id = Uuid::new_v4().to_string();
    info!(
        "Received Inform message from {:?}, generated new session-id: {}",
        cpe_addr, session_id
    );
    // Create a new session state and insert it into the DashMap
    // Create the command channel for the session
    let (command_tx, command_rx) = mpsc::channel::<AcsCommand>(16);
    let session_state = SessionState::new(
        device_id.clone(),
        correlation_id.clone(),
        source.clone(),
        (*NATS_MY_COMMANDS_SUBJECT).clone(),
        command_rx,
        command_tx,
    );
    session_map.insert(session_id.clone(), Arc::new(Mutex::new(session_state)));

    if let Source::Api = source {
        // We should NOT send the inform message to the backend
        debug!("Handling API initiated inform message, meaning do nothing here and wait for the empty post");
    } else {
        // We should send the inform message to the backend
        // and wait for a response
        debug!("Handling CPE initiated inform message");
        // Send the Inform message to the backend
        let event_type = datamodel::EventType::Inform(datamodel::InformMessage::from(inform));
        let acs_event = datamodel::AcsEvent {
            session_id: session_id.to_string(),
            device_id,
            correlation_id,
            source,
            event_type,
        };

        match serde_json::to_string(&acs_event) {
            Ok(acs_event) => {
                info!("Generated backend event: {}", acs_event);
                // Send it to the backend, and wait for a response or a timeout
                // No matter what, if this was an Inform message, we should include an InformResponse in the response
                match notify_backend(
                    nats,
                    &*NATS_EVENTS_SUBJECT,
                    &session_id,
                    &(*NATS_MY_COMMANDS_SUBJECT).clone(),
                    &acs_event,
                )
                .await
                {
                    Ok(()) => {
                        debug!("Sent Inform message to backend");
                    }
                    Err(e) => {
                        error!(
                        "Failed to send Inform message to backend: {:?}, session will probably be terminated early",
                        e
                    );
                    }
                }
            }
            Err(e) => {
                error!(
                    "Failed to generate backend event: {:?}, session will probably terminate early",
                    e
                );
            }
        }
    }

    let id = header_element
        .and_then(|header| match header {
            HeaderElement::ID(id) => Some(id.clone()),
            _ => None,
        })
        .unwrap_or_else(|| cwmp::protocol::ID::new(false, ""));

    let mut response: warp::http::Response<Body> = Response::builder()
        .status(warp::http::StatusCode::OK)
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(build_plain_inform_response_body(&id))
        .unwrap();

    // Set the session cookie in the response
    let cookie_header = format!("session={session_id}; HttpOnly; Path=/; Max-Age=3600");
    let headers = response.headers_mut();
    headers.append(header::SET_COOKIE, cookie_header.parse().unwrap());
    Ok(Box::new(response))
}

/// This is where we see for instance `GetParameterValuesResponse` aso
///
/// We just need to send the response to the backend, and wait for the next command
/// or terminate the session by sending an empty response
async fn handle_non_inform_post(
    nats: &NatsClient,
    session_id: &str,
    session_map: SessionMap,
    body_element: &BodyElement,
    redis_manager: Arc<RedisManager>,
) -> Result<Box<dyn warp::Reply>, warp::Rejection> {
    if let Some(session_entry) = session_map.get(session_id) {
        let session = Arc::clone(&session_entry);

        let (source, device_id, reply_to) = {
            let state = session.lock().await;
            (
                state.source.clone(),
                state.device_id.clone(),
                state.reply_to.clone(),
            )
        };

        if let Source::Api = source {
            // A response arrived for an API session.
            // Send the response to the API consumer
            // Remove and terminate the session
            debug!("Session is an API session, sending response to API consumer");
            drop(session_entry);
            session_map.remove(session_id);
            // Remove the command from the cache
            match redis_manager
                .delete_key(&format!("acs:commands:{device_id}"))
                .await
            {
                Ok(()) => debug!("Command removed pending command from cache"),
                Err(e) => {
                    error!("Failed to remove pending command from cache: {:?}", e);
                }
            }

            let state = session.lock().await;

            match generate_backend_event(session_id, &state, Some(body_element)) {
                Ok(event_json) => {
                    // Send the event to the API consumer
                    match send_api_response(reply_to.into(), &device_id, &event_json).await {
                        Ok(()) => {
                            debug!("Sent event to API consumer: {}", event_json);
                        }
                        Err(e) => {
                            error!("Failed to send event to API consumer: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to generate backend event: {:?}, terminating session",
                        e
                    );
                    return Ok(Box::new(
                        Response::builder()
                            .header("Content-Type", "text/xml; charset=utf-8")
                            .body(Body::empty())
                            .unwrap(),
                    ));
                }
            }

            // Send the empty response to the device, in order to terminate the session
            Ok(Box::new(
                Response::builder()
                    .header("Content-Type", "text/xml; charset=utf-8")
                    .body(Body::empty())
                    .unwrap(),
            ))
        } else {
            // Step 1: Take the receiver
            let (command_rx, backend_event, request_id, device_controller_subject) = {
                let mut state = session.lock().await;
                let new_outstanding_request_id = uuid::Uuid::new_v4().to_string();
                state.last_seen = Some(tokio::time::Instant::now());
                state.outstanding_request_id = Some(new_outstanding_request_id.clone());
                match generate_backend_event(session_id, &state, Some(body_element)) {
                    Ok(backend_event) => {
                        info!("Generated backend event: {}", backend_event);
                        (
                            state.command_rx.take(),
                            backend_event,
                            new_outstanding_request_id,
                            state.reply_to.clone(),
                        )
                    }
                    Err(e) => {
                        error!(
                            "Failed to generate backend event: {:?}, termintating session",
                            e
                        );
                        session_map.remove(session_id);

                        return Ok(Box::new(
                            Response::builder()
                                .header("Content-Type", "text/xml; charset=utf-8")
                                .body(Body::empty())
                                .unwrap(),
                        ));
                    }
                }
            };

            if let Some(mut response_rx) = command_rx {
                // Send it to the backend, and wait for a response or a timeout
                // A non-inform message (a response) should be sent towards the device controller that sent the command
                // down to this session handler, and the reply_to should be the subject that this session handler
                // is listening on for commands
                if let Ok(()) = notify_backend(
                    nats,
                    &device_controller_subject,
                    session_id,
                    &(*&NATS_MY_COMMANDS_SUBJECT).clone(),
                    &backend_event,
                )
                .await
                {
                    let response_body = match tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        response_rx.recv(),
                    )
                    .await
                    {
                        Ok(Some(response)) => {
                            // A response from kafka was returned, this must be parsable as a command
                            // and handled accordingly
                            debug!(
                                "Received a response from backend in the handler channel: {:?}",
                                response
                            );
                            build_cpe_response_body(&response, &request_id)
                        }
                        Ok(None) => {
                            // The channel to process kafka message had a problem
                            error!("Received None from the kafka channel");
                            // Terminate session
                            Body::empty()
                        }
                        Err(_) => {
                            // Here we should probably just reply 200 OK, to terminate the session with the device
                            error!("Timeout waiting for response from upstream");
                            Body::empty()
                        }
                    };
                    let mut state = session.lock().await;
                    state.command_rx = Some(response_rx);
                    debug!("Sending response to CPE: {:?}", response_body);
                    let cookie_header =
                        format!("session={session_id}; HttpOnly; Path=/; Max-Age=3600");
                    // Set the connection header to keep-alive
                    let response_builder = Response::builder()
                        .header("Content-Type", "text/xml; charset=utf-8")
                        .header(header::CONNECTION, "keep-alive")
                        .header(header::SET_COOKIE, cookie_header);

                    // Return the response
                    Ok(Box::new(response_builder.body(response_body).unwrap()))
                } else {
                    error!("Could not notify backend of event, terminating session");
                    session_map.remove(session_id);
                    Ok(Box::new(
                        Response::builder()
                            .header("Content-Type", "text/xml; charset=utf-8")
                            .body(Body::empty())
                            .unwrap(),
                    ))
                }
            } else {
                // No command receiver available, session is closed
                debug!("No command receiver available, session is closed");
                session_map.remove(session_id);
                Ok(Box::new(
                    Response::builder()
                        .header("Content-Type", "text/xml; charset=utf-8")
                        .body(Body::empty())
                        .unwrap(),
                ))
            }
        }
    } else {
        // Session not found — reject or send basic empty response
        debug!("Session not found, closing session towards CPE");
        Ok(Box::new(
            Response::builder()
                .header("Content-Type", "text/xml; charset=utf-8")
                .body(Body::empty())
                .unwrap(),
        ))
    }
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
) -> Result<Box<dyn warp::Reply>, warp::Rejection> {
    info!("Received CWMP request. Cookie: {:?}", cookie);

    if let Some(session_cookie) = cookie {
        // ── Ongoing session ──────────────────────────────────────────────────
        // Cookie present: the device is mid-session.
        info!("Session Cookie: {:?}", session_cookie);

        if body.is_empty() {
            // Case 2: Empty POST — device is ready for the next command.
            // No parsing needed; just dispatch the next queued command.
            info!("Empty POST on existing session — sending next command");
            return handle_empty_post(&session_cookie, session_cookie.clone(), state).await;
        } else {
            // Case 3: Command response — parse and handle, then send next command.
            match cwmp::parse_bytes(body.as_ref()) {
                Ok(parsed_envelope) => {
                    info!("Command response: {:?}", parsed_envelope);
                    // A CWMP response body has exactly one element (e.g. GetParameterValuesResponse).
                    if let Some(body_element) = parsed_envelope.body.first() {
                        return handle_non_inform_post(
                            &state.nats,
                            &session_cookie,
                            state.sessions.clone(),
                            body_element,
                            state.redis.clone(),
                        )
                        .await;
                    }
                    // Empty body after parse — nothing to dispatch, fall through to stub.
                }
                Err(e) => {
                    error!("Error parsing command response: {:?}", e);
                    return Ok(Box::new(warp::reply::with_status(
                        "Error parsing XML",
                        warp::http::StatusCode::BAD_REQUEST,
                    )));
                }
            }
        }
    } else {
        // ── No cookie: must be an Inform ─────────────────────────────────────
        // An empty body here is a protocol error — nothing to do without a session.
        if body.is_empty() {
            error!("Empty body with no session cookie — rejecting");
            return Ok(Box::new(warp::reply::with_status(
                "Bad Request",
                warp::http::StatusCode::BAD_REQUEST,
            )));
        }

        // Case 1: Parse and expect an Inform to start a new session.
        match cwmp::parse_bytes(body.as_ref()) {
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
                    return handle_inform_post(
                        &state.nats,
                        None,
                        state.sessions.clone(),
                        &inform,
                        None,
                        state.redis.clone(),
                    )
                    .await;
                } else {
                    error!("Non-Inform body received without a session cookie — rejecting");
                    return Ok(Box::new(warp::reply::with_status(
                        "Bad Request",
                        warp::http::StatusCode::BAD_REQUEST,
                    )));
                }
            }
            Err(e) => {
                error!("Error parsing Inform: {:?}", e);
                return Ok(Box::new(warp::reply::with_status(
                    "Error parsing XML",
                    warp::http::StatusCode::BAD_REQUEST,
                )));
            }
        }
    }

    Ok(Box::new(warp::reply::with_status(
        "CWMP Handler Stub",
        warp::http::StatusCode::OK,
    )))
}
