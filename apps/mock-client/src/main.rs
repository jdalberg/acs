use chrono::Utc;
use cwmp::protocol::BodyElement;
use cwmp::protocol::CwmpVersion;
use cwmp::protocol::Fault;
use lazy_static::lazy_static;
use log::{debug, error, info};
use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::header::COOKIE;
use reqwest::Client;
use std::env;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};
use warp::Filter;

mod device_info;

lazy_static! {
    static ref DEVICE_INFO: Arc<Mutex<device_info::DeviceInfo>> =
        Arc::new(Mutex::new(device_info::DeviceInfo::new()));
}

#[derive(Debug)]
struct ParameterValue {
    name: String,
    value: String,
}

// This is called whenever a response is received from the ACS server, directly after the reqwest client
// receives the response. It should process the response and possibly queue a new request to the server
// in the command channel.
async fn handle_acs_response(response: &str, command_tx: Sender<String>) {
    // Here we should process the ACS response and generate a new request for the server
    // with the result.
    // For now, we just print the response.
    debug!("Handling ACS response: [{}]", response);

    if response.contains("GetParameterValues") {
        if let Err(e) = command_tx.send(response.to_string()).await {
            error!("Failed to send command: {}", e);
        }
    } else if response.contains("SetParameterValues") {
        if let Err(e) = command_tx.send(response.to_string()).await {
            error!("Failed to send command: {}", e);
        }
    } else if response.contains("GetParameterNames") {
        if let Err(e) = command_tx.send(response.to_string()).await {
            error!("Failed to send command: {}", e);
        }
    } else if response.contains("Fault") {
        debug!("Fault response received: {}", response);
        DEVICE_INFO.lock().await.end_session();
    } else if response.contains("InformResponse") {
        // According to TR-069, after InformResponse we must send an Empty POST
        if let Err(e) = command_tx.send("EMPTY_POST".to_string()).await {
            error!("Failed to queue empty post: {}", e);
        }
    } else if response.is_empty() {
        DEVICE_INFO.lock().await.end_session();
    } else {
        error!("Unknown command received: {response}");
    }
}

// Send a message to the ACS server
async fn send_message(message_body: &str, o_command_tx: Option<Sender<String>>) {
    let client = Client::new();
    let acs_url = DEVICE_INFO.lock().await.acs_url.clone();
    info!("Sending message to {acs_url}");
    debug!("Message body: {}", message_body);
    // Create a headers map and set the "Cookie" header
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", HeaderValue::from_static("text/xml"));
    if let Some(cookie) = DEVICE_INFO.lock().await.get_session_cookie() {
        debug!("Sending session cookie: {}", cookie);
        headers.insert(COOKIE, HeaderValue::from_str(&cookie).unwrap());
    }
    let body = message_body.to_string();
    match client
        .post(&acs_url)
        .headers(headers)
        .body(body)
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            // Extract the "Set-Cookie" header
            if let Some(cookie_header) = response.headers().get(reqwest::header::SET_COOKIE) {
                debug!("Received Set-Cookie: {:?}", cookie_header.to_str().unwrap());
                // Store the cookie for subsequent requests
                if let Some(cookie) = cookie_header.to_str().ok() {
                    DEVICE_INFO.lock().await.start_session(cookie.to_string())
                }
            }
            match response.text().await {
                Ok(text) => {
                    debug!("Response message received: {}", status);
                    // This is where the magic happens
                    if let Some(command_tx) = o_command_tx {
                        handle_acs_response(&text, command_tx).await;
                    }
                }
                Err(e) => error!("Failed to read response text: {}", e),
            }
        }
        Err(e) => {
            println!("Failed to send GetParameterValuesResponse: {}", e);
        }
    }
}

async fn send_fault(code: i32, string: &str, header: Vec<cwmp::protocol::HeaderElement>) {
    let message_body = cwmp::generate(&cwmp::protocol::Envelope {
        cwmp_version: Some(CwmpVersion::new(1, 0)),
        header,
        body: vec![BodyElement::Fault(Fault::new(
            &code.to_string(),
            string,
            0,
            "",
        ))],
    })
    .unwrap();

    send_message(&message_body, None).await;
}

// This is simply the connection request handler. All it should do is
// to return a 200 OK response with the content type text/xml
// and the body of the response should be empty.
// The CPE should then send a POST request to the ACS URL with the
// Inform message.
async fn handle_request(
    inform_scheduler_tx: Sender<Vec<String>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    if let Err(e) = inform_scheduler_tx
        .send(vec!["6 CONNECTION REQUEST".to_string()])
        .await
    {
        error!("Failed to send inform into the scheduler: {}", e);
    }
    Ok(warp::reply())
}

fn parse_parameter_names(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut parameters = Vec::new();
    let mut in_parameter_name = false;

    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if e.name() == b"ParameterNames" || e.name() == b"string" {
                    in_parameter_name = true;
                }
            }
            Ok(Event::Text(e)) if in_parameter_name => {
                if let Ok(name) = e.unescape_and_decode(&reader) {
                    parameters.push(name);
                }
            }
            Ok(Event::End(ref e)) => {
                if e.name() == b"ParameterNames" || e.name() == b"string" {
                    in_parameter_name = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                println!("Error parsing XML: {:?}", e);
                break;
            }
            _ => (),
        }
        buf.clear();
    }
    parameters
}

fn parse_parameter_values(xml: &str) -> Vec<ParameterValue> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut parameters = Vec::new();
    let mut current_name = None;
    let mut current_value = None;

    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name() {
                b"Name" => {
                    if let Ok(Event::Text(e)) = reader.read_event(&mut buf) {
                        if let Ok(name) = e.unescape_and_decode(&reader) {
                            current_name = Some(name);
                        }
                    }
                }
                b"Value" => {
                    if let Ok(Event::Text(e)) = reader.read_event(&mut buf) {
                        if let Ok(value) = e.unescape_and_decode(&reader) {
                            current_value = Some(value);
                        }
                    }
                }
                _ => (),
            },
            Ok(Event::End(ref e)) => {
                if e.name() == b"ParameterValueStruct" {
                    if let (Some(name), Some(value)) = (current_name.take(), current_value.take()) {
                        parameters.push(ParameterValue { name, value });
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                println!("Error parsing XML: {:?}", e);
                break;
            }
            _ => (),
        }
        buf.clear();
    }
    parameters
}

fn extract_id_header(xml: &str) -> Vec<cwmp::protocol::HeaderElement> {
    if let Ok(envelope) = cwmp::parse_bytes(xml.as_bytes()) {
        envelope.header.into_iter().filter(|h| matches!(h, cwmp::protocol::HeaderElement::ID(_))).collect()
    } else {
        vec![]
    }
}

fn generate_fault(code: i32, string: &str, header: Vec<cwmp::protocol::HeaderElement>) -> String {
    cwmp::generate(&cwmp::protocol::Envelope {
        cwmp_version: Some(CwmpVersion::new(1, 0)),
        header,
        body: vec![BodyElement::Fault(Fault::new(
            &code.to_string(),
            string,
            0,
            "",
        ))],
    })
    .unwrap()
}

fn generate_envelope(header: Vec<cwmp::protocol::HeaderElement>, body_element: BodyElement) -> String {
    cwmp::generate(&cwmp::protocol::Envelope {
        cwmp_version: Some(CwmpVersion::new(1, 0)),
        header,
        body: vec![body_element],
    })
    .unwrap()
}

async fn send_get_parameter_names_response(
    get_parameter_names_xml: &str,
    command_tx: Sender<String>,
) {
    // Just mock a few parameters at the root level for testing
    let parameter_list = vec![
        cwmp::protocol::ParameterInfoStruct::new("Device.", 1),
        cwmp::protocol::ParameterInfoStruct::new("Device.DeviceInfo.", 1),
        cwmp::protocol::ParameterInfoStruct::new("Device.DeviceInfo.Manufacturer", 1),
        cwmp::protocol::ParameterInfoStruct::new("Device.DeviceInfo.HardwareVersion", 1),
        cwmp::protocol::ParameterInfoStruct::new("Device.DeviceInfo.SoftwareVersion", 1),
    ];
    let header = extract_id_header(get_parameter_names_xml);
    let xml = generate_envelope(header, BodyElement::GetParameterNamesResponse(
        cwmp::protocol::GetParameterNamesResponse::new(parameter_list),
    ));
    send_message(&xml, Some(command_tx)).await;
}

async fn send_get_parameter_values_response(
    get_parameter_values_xml: &str,
    command_tx: Sender<String>,
) {
    let parameter_names = parse_parameter_names(get_parameter_values_xml);

    let xml = {
        let info = DEVICE_INFO.lock().await;
        let parameter_values: Vec<cwmp::protocol::ParameterValue> = parameter_names
            .iter()
            .filter_map(|name| {
                let value = info.get_parameter_value(name);
                if let Some(v) = value {
                    Some(cwmp::protocol::ParameterValue::new(name, "STRING", &v))
                } else {
                    None
                }
            })
            .collect();
        if parameter_values.is_empty() {
            generate_fault(9005, "Parameter not found", extract_id_header(get_parameter_values_xml))
        } else {
            generate_envelope(extract_id_header(get_parameter_values_xml), BodyElement::GetParameterValuesResponse(
                cwmp::protocol::GetParameterValuesResponse::new(parameter_values),
            ))
        }
    };
    send_message(&xml, Some(command_tx)).await;
}

// If we found any parameters, return them
// If we didn't find any parameters, return a Fault
async fn send_set_parameter_values_response(
    set_parameter_values_xml: &str,
    command_tx: Sender<String>,
) {
    let parameters = parse_parameter_values(set_parameter_values_xml);
    let mut status: u32 = 0;
    let mut found_something = false;

    let xml = {
        let mut info = DEVICE_INFO.lock().await;
        // Try to set all parameters
        for param in parameters {
            match info.set_parameter_value(&param.name, &param.value) {
                Ok(()) => {
                    status = 0;
                    found_something = true;
                }
                Err(error_code) => {
                    status = error_code;
                    error!(
                        "Failed to set parameter {}: error code {}",
                        param.name, error_code
                    );
                    break; // Stop at first error as per TR-069
                }
            }
        }

        if found_something {
            generate_envelope(extract_id_header(set_parameter_values_xml), BodyElement::SetParameterValuesResponse(
                cwmp::protocol::SetParameterValuesResponse::new(status),
            ))
        } else {
            generate_fault(9005, "Parameter not found", extract_id_header(set_parameter_values_xml))
        }
    };

    send_message(&xml, Some(command_tx)).await;
}

async fn send_inform(events: Vec<String>, command_tx: Sender<String>) {
    let xml = {
        let info = DEVICE_INFO.lock().await.clone();
        let current_time = Utc::now();
        let parameters = [
            ("Device.ManagementServer.URL", &info.acs_url),
            (
                "Device.ManagementServer.ConnectionRequestURL",
                &info.connection_request_url,
            ),
            ("Device.ManagementServer.ParameterKey", &info.parameter_key),
            ("Device.DeviceInfo.HardwareVersion", &info.hardware_version),
            ("Device.DeviceInfo.SoftwareVersion", &info.software_version),
        ];

        let parameter_list: Vec<cwmp::protocol::ParameterValue> = parameters
            .iter()
            .map(|(name, value)| cwmp::protocol::ParameterValue::new(name, "STRING", value))
            .collect();

        let events: Vec<cwmp::protocol::EventStruct> = events
            .iter()
            .map(|event| cwmp::protocol::EventStruct::new(&event, &info.parameter_key))
            .collect::<Vec<_>>();

        //        new(device_id: DeviceId, event: Vec<EventStruct>, max_envelopes: u32, current_time: DateTime<Utc>, retry_count: u32, parameter_list: Vec<ParameterValue>)

        cwmp::generate(&cwmp::protocol::Envelope {
            cwmp_version: Some(CwmpVersion::new(1, 0)),
            header: vec![],
            body: vec![BodyElement::Inform(cwmp::protocol::Inform::new(
                cwmp::protocol::DeviceId::new(
                    &info.manufacturer.clone(),
                    &info.oui.clone(),
                    &info.product_class.clone(),
                    &info.serial_number.clone(),
                ),
                events,
                1,
                current_time,
                0,
                parameter_list,
            ))],
        })
        .unwrap()
    };

    send_message(&xml, Some(command_tx)).await;
}

async fn send_periodic_inform_task(command_tx: Sender<String>) {
    let mut interval = time::interval(Duration::from_secs(600));
    let mut nof_ticks = 0;
    loop {
        interval.tick().await;

        if nof_ticks > 0 {
            send_inform(vec![String::from("2 PERIODIC")], command_tx.clone()).await;
        }
        nof_ticks += 1;
    }
}

async fn process_command_task(
    mut commands_rx: mpsc::Receiver<String>,
    commands_tx: Sender<String>,
) {
    loop {
        match commands_rx.recv().await {
            Some(command) => {
                tokio::time::sleep(Duration::from_secs(1)).await;

                if command.contains("GetParameterValues") {
                    debug!("Processing command: GetParameterValues");
                    send_get_parameter_values_response(&command, commands_tx.clone()).await;
                    debug!("GetParameterValues command processed: {}", command);
                } else if command.contains("SetParameterValues") {
                    debug!("Processing command: SetParameterValues");
                    send_set_parameter_values_response(&command, commands_tx.clone()).await;
                    debug!("SetParameterValues command processed: {}", command);
                } else if command.contains("GetParameterNames") {
                    debug!("Processing command: GetParameterNames");
                    send_get_parameter_names_response(&command, commands_tx.clone()).await;
                    debug!("GetParameterNames command processed: {}", command);
                } else if command == "EMPTY_POST" {
                    debug!("Sending Empty POST...");
                    send_message("", Some(commands_tx.clone())).await;
                } else {
                    send_fault(9000, "Method not supported", extract_id_header(&command)).await;
                    debug!("Unknown command: {}", command);
                }
                // Here we should process the command and generate a new request for the server
                // with the result.
                // For now, we just print the command.
                info!("Command processed: {}", command);
            }
            None => {
                debug!("No more commands to process");
                break;
            }
        }
    }
}

async fn inform_scheduler_task(mut informs_rx: Receiver<Vec<String>>, command_tx: Sender<String>) {
    loop {
        let inform_events = informs_rx.recv().await;
        if let Some(events) = inform_events {
            debug!("Received inform to send: {:?}", events);
            send_inform(events, command_tx.clone()).await;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

fn with_scheduler_tx(
    inform_scheduler_tx: mpsc::Sender<Vec<String>>,
) -> impl Filter<Extract = (mpsc::Sender<Vec<String>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || inform_scheduler_tx.clone())
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let port = env::var("CPE_LISTEN_PORT").unwrap_or_else(|_| "7547".to_string());

    // Create a channel for sending commands
    let (commands_tx, commands_rx) = mpsc::channel::<String>(10);
    let (informs_tx, informs_rx) = mpsc::channel::<Vec<String>>(10);
    let commands_tx_clone = commands_tx.clone();
    let commands_tx_sheduler_clone = commands_tx.clone();
    tokio::spawn(async move { send_periodic_inform_task(commands_tx).await });
    tokio::spawn(async move { process_command_task(commands_rx, commands_tx_clone).await });
    tokio::spawn(async move {
        inform_scheduler_task(informs_rx, commands_tx_sheduler_clone.clone()).await
    });

    let route = warp::get()
        .and(with_scheduler_tx(informs_tx.clone()))
        .and_then(handle_request);

    info!("CWMP Mock CPE started on 127.0.0.1:{}", port);
    let warp_task = warp::serve(route).run(([127, 0, 0, 1], port.parse().unwrap()));

    if let Err(e) = informs_tx.send(vec![String::from("1 BOOT")]).await {
        error!("Failed to send BOOT inform into the scheduler: {}", e);
    }

    warp_task.await;
}