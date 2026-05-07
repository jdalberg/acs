use env_logger::init;
use log::{error, info};
use models::{EventMessage, PolicyMessage};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::Message;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use tokio::time::Duration;
use tokio_stream::StreamExt;

mod errors;
mod handlers;
mod models;
mod routes;

/// Consume policy events from a Kafka topic, these events are produced by Core
/// when a new session is created or updated.
///
/// Send the received events through a tokio channel to the main thread.
///
/// # Arguments
///     * `topic` - The Kafka topic to consume messages from
///     * `partition` - The partition to consume messages from, defaults to all. The idea is to have a stateful set decide the partition for each pod.
///     * `tx` - The tokio channel sender to send the received messages to the main thread
async fn run_queue_consumer(
    topic: &str,
    partition: u16,
    tx: tokio::sync::mpsc::Sender<PolicyMessage>,
) {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", "127.0.0.1:29092")
        .set("group.id", "nuuday-acs-instances")
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "true")
        .create()
        .expect("Consumer creation failed");

    consumer
        .subscribe(&[topic])
        .expect("Failed to subscribe to topic");

    let mut message_stream = consumer.stream();

    info!("Consuming policy event messages...");

    while let Some(result) = message_stream.next().await {
        match result {
            Ok(borrowed_message) => {
                let payload = borrowed_message.payload().unwrap_or(&[]);
                println!("Received message: {:?}", String::from_utf8_lossy(payload));

                // You can manually commit offsets or rely on auto-commit
            }
            Err(e) => {
                eprintln!("Error while receiving message: {:?}", e);
            }
        }
    }
}

async fn run_event_producer(
    upstream_event_topic: &str,
    upstream_event_partition: u16,
    mut rx: tokio::sync::mpsc::Receiver<EventMessage>,
) {
    // Create the Kafka producer
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", "localhost:29092")
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Producer creation error");

    info!("Starting event producer task...");

    // Listen for messages on the Receiver channel
    while let Some(event) = rx.recv().await {
        let key = event.kafka_key();
        let payload = event.as_kafka_message();

        let produce_result = producer
            .send(
                FutureRecord::to(upstream_event_topic)
                    .key(&key)
                    .payload(&payload),
                Duration::from_secs(0),
            )
            .await;

        match produce_result {
            Ok(delivery) => println!("Message delivered: {:?}", delivery),
            Err((e, _)) => eprintln!("Error while sending message: {:?}", e),
        }
    }
}

async fn empty_queues() {
    // Empty the queues
}

async fn run_web_server(
    controller_endpoint: &str,
    event_tx: tokio::sync::mpsc::Sender<EventMessage>,
) {
    let routes = routes::build_routes(event_tx);

    info!("Starting web service task...");

    // Start the web server
    warp::serve(routes).run(([127, 0, 0, 1], 8080)).await;
}

#[tokio::main]
async fn main() {
    init();
    // Create tokio channels for the web server and Kafka consumer tasks
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(100);
    let (policy_tx, mut policy_rx) = tokio::sync::mpsc::channel(100);

    // Read the queue topic and partiopn from the environment
    let policy_events_queue_topic = std::env::var("KAFKA_POLICY_QUEUE_TOPIC")
        .unwrap_or_else(|_| "nuuacs-policy-events".to_string());
    let policy_event_queue_partition = std::env::var("KAFKA_POLICY_QUEUE_PARTITION")
        .unwrap_or_else(|_| "0".to_string())
        .parse::<u16>()
        .unwrap_or(0);

    let incoming_events_topic = std::env::var("KAFKA_INFORM_EVENTS_TOPIC")
        .unwrap_or_else(|_| "nuuacs-inform-events".to_string());
    let incoming_events_partition = std::env::var("KAFKA_INFORM_EVENTS_PARTITION")
        .unwrap_or_else(|_| "0".to_string())
        .parse::<u16>()
        .unwrap_or(0);

    // Read the configuration controller endpoint from the environment
    let controller_endpoint = std::env::var("CONTROLLER_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    let pod_name = std::env::var("POD_NAME").unwrap_or_else(|_| String::from("dev_pod"));

    // Create a new Kafka comsumer task
    let mut queue_consumer = Box::pin(run_queue_consumer(
        &policy_events_queue_topic,
        policy_event_queue_partition,
        policy_tx,
    ));

    // Run the web server task to take new sessions from the network
    let mut web_server = Box::pin(run_web_server(&controller_endpoint, event_tx));

    // Run the event producer
    let mut event_producer = Box::pin(run_event_producer(
        &incoming_events_topic,
        incoming_events_partition,
        event_rx,
    ));

    // Setup the channel interactions
    loop {
        tokio::select! {
            q_result = &mut queue_consumer => {
                // queue consumer died, log the error and stop all other tasks then exit
                error!("Queue consumer task failed: {:?}", q_result);
                break;
            },
            w_result = &mut web_server => {
                // queue consumer died, log the error and stop all other tasks then exit
                error!("Web server task failed: {:?}", w_result);
                break;
            },
            e_result = &mut event_producer => {
                // event producer died, log the error and stop all other tasks then exit
                error!("Event producer task failed: {:?}", e_result);
                break;
            },
            Some(policy_event) = policy_rx.recv() => {
                // Send the policy event to the web server
                info!("Received policy event: {:?}", policy_event);
            },

        }
    }
}
