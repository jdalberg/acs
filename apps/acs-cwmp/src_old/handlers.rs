use crate::models::EventMessage;
use log::error;
use tokio::time::{timeout, Duration};

const MAX_INFORM_TIMEOUT: u64 = 20;

// This function is used to empty the policy and event queues
// for a specific device from kafka.
// A timeout or an END_OF_POLICIES marker will denote an end of the queue.
async fn empty_queues() {
    // Do nothing
}

// An async handler that simulates a delayed response with a timeout
pub async fn inform(
    xml: bytes::Bytes,
    event_tx: tokio::sync::mpsc::Sender<EventMessage>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let timeout_duration: Duration = Duration::from_secs(MAX_INFORM_TIMEOUT);

    // Parse the incoming XML payload
    match cwmp::parse_bytes(xml.as_ref()) {
        Ok(parsed_envelope) => {
            // handle the parsed envelope if it is an inform
            if parsed_envelope.is_inform() {
                // Send the event upstream
                match EventMessage::try_from(&parsed_envelope) {
                    Ok(event_message) => {
                        // Set a timeout for sending the message
                        let send_result = event_tx.send(event_message).await;

                        match send_result {
                            Ok(()) => {
                                empty_queues().await;
                                Ok(warp::reply::with_status("", warp::http::StatusCode::OK))
                            }
                            Err(_) => Ok(warp::reply::with_status(
                                "Channel send failed",
                                warp::http::StatusCode::REQUEST_TIMEOUT,
                            )),
                        }
                    }
                    Err(e) => {
                        error!("Error parsing inform message: {:?}", e);
                        Ok(warp::reply::with_status(
                            "Error parsing inform message",
                            warp::http::StatusCode::BAD_REQUEST,
                        ))
                    }
                }
            } else {
                Ok(warp::reply::with_status(
                    "Not an Inform message",
                    warp::http::StatusCode::BAD_REQUEST,
                ))
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
}

// Another async handler that returns a simple hello message
pub async fn hello() -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::with_status(
        "Hello, world!",
        warp::http::StatusCode::OK,
    ))
}
