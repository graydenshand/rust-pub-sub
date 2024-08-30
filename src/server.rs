use rmpv::Value;
use std::collections::HashMap;

use log::{debug, info, warn};
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

use tokio;
use tokio::sync::mpsc;

use tokio::net::tcp::OwnedReadHalf;
use tokio::net::TcpListener;

use crate::config;
use crate::datagram::{Message, MessageReader, MessageWriter};
use crate::subscription_tree::{self};

/// An async message passing application
#[derive(Debug, Clone)]
pub struct Server {
    /// Port on which to listen for new requests
    port: u16,
    ///  Client subscriptions on this server
    subscribers: Arc<Mutex<subscription_tree::SubscriptionTree<String>>>,
    /// Client channel map
    write_channel_map: Arc<Mutex<HashMap<String, mpsc::Sender<Message>>>>,
}
impl Server {
    pub async fn new(port: u16) -> Result<Server, Box<dyn Error>> {
        Ok(Server {
            port,
            subscribers: Arc::new(Mutex::new(subscription_tree::SubscriptionTree::new())),
            write_channel_map: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Process a message published by a client
    async fn on_receive(&self, client_id: &String, message: Message) {
        let topic = message.topic();
        let value = message.value();
        // Handle system messages
        if message.topic().starts_with(config::SYSTEM_TOPIC_PREFIX) {
            match message.topic().trim_start_matches(config::SYSTEM_TOPIC_PREFIX) {
                config::SUBSCRIBE_TOPIC => {
                    // Message value contains subscription pattern
                    debug!(
                        "SUBSCRIBE - {} - {}",
                        client_id.to_string(),
                        &message.value().as_str().unwrap()
                    );
                    // Wait for ownership of mutex lock
                    let mut subscribers = self.subscribers.lock().unwrap();

                    // Add new subscription entry to the subscriber tree
                    subscribers.subscribe(&message.value().as_str().unwrap(), client_id.to_string());
                }
                _ => {
                    warn!("Message published to unrecognized system topic: {topic}");
                }
            }
        } else {
            debug!("PUBLISH - {client_id} - {topic} {value}");
        };

        let subscribers = self.subscribers.lock().unwrap().get_subscribers(message.topic());
        let write_channels = subscribers.iter().map(|client_id| {
            if let Some(tx) = self.write_channel_map.lock().unwrap().get(client_id) {
                tx.clone()
            } else {
                panic!("Inconsistent state between subscribers and write_channel_map");
            }
        });

        for tx in write_channels {
            tx.send(message.clone()).await.ok();
        }
    }

    /// Maintain connection with a client and handle published messages
    pub async fn receive_loop(&self, client_id: String, stream: OwnedReadHalf) -> () {
        let mut reader = MessageReader::new(stream);
        let start = Instant::now();

        let mut count = 0;

        loop {
            let message = reader.read_value().await.ok();
            if message.is_none() || message.as_ref().unwrap().is_none() {
                // Unsubscribe client from all topics
                let _ = self
                    .subscribers
                    .lock()
                    .unwrap()
                    .unsubscribe_client(&client_id);

                // Log stats about messages received from client
                let end = Instant::now();
                let seconds = (end - start).as_millis() as f64 / 1000.0;
                info!(
                    "DISCONNECT - {client_id} - {count} messages received in {seconds}s - {} m/s",
                    (count as f64 / seconds).round()
                );
                // Terminate loop
                return;
            } else {
                let m: Message = message
                    .expect("message is Ok")
                    .expect("message is not None");

                self.on_receive(&client_id, m).await;
            }

            count += 1;
        }
    }

    /// Listen for incoming connections
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port)).await?;

        loop {
            let (stream, _) = listener.accept().await?;
            let client_id = Uuid::new_v4();
            info!("CONNECT - {}", client_id.to_string());
            let (r, w) = stream.into_split();

            let server_clone = self.clone();
            tokio::spawn(async move {
                _ = server_clone.receive_loop(client_id.to_string(), r).await;
            });

            let mut writer = MessageWriter::new(w);
            let (tx, rx) = mpsc::channel(config::CHANNEL_BUFFER_SIZE);
            let tx1 = tx.clone();
            self.write_channel_map
                .lock()
                .unwrap()
                .insert(client_id.to_string(), tx1);
            tokio::spawn(async move {
                writer.subscribe_to_channel(rx).await.ok();
            });

            let tx2 = tx.clone();
            tokio::spawn(async move {
                loop {
                    tx2.send(Message::new(
                        &format!("{}{}", config::SYSTEM_TOPIC_PREFIX, config::HEALTH_TOPIC),
                        Value::from("Ok"),
                    ))
                    .await
                    .ok();
                    tokio::time::sleep(Duration::from_secs(config::HEALTH_CHECK_INTERVAL_S)).await;
                }
            });
        }
    }
}
