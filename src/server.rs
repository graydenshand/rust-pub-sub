use rmpv::Value;
use std::collections::HashMap;
use tonic::client;

use chrono::DateTime;
use std::alloc::System;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

use tokio;
use tokio::sync::mpsc;

use tokio::net::tcp::OwnedReadHalf;
use tokio::net::TcpListener;

use crate::datagram::{
    Message, MessageReader, MessageWriter, SUBSCRIBE_TOPIC, SYSTEM_TOPIC_PREFIX,
};
use crate::subscription_tree::{self};

#[derive(Debug, Clone)]
struct Subscription {
    addr: String,
    pattern: String,
}

/// An async message passing application
#[derive(Debug, Clone)]
pub struct Server {
    /// Port on which to listen for new requests
    port: u16,
    /// Structure containing information about clients subscribed to topics on this Server
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
    async fn on_receive(&self, client_id: &String, m: Message) {
        println!(
            "{:?} Message received - {client_id} - {} - {}",
            chrono::offset::Local::now(),
            m.topic(),
            m.value()
        );
        // Handle system messages
        if m.topic().starts_with(SYSTEM_TOPIC_PREFIX) {
            match m.topic().trim_start_matches(SYSTEM_TOPIC_PREFIX) {
                SUBSCRIBE_TOPIC => {
                    // Message value contains subscription pattern
                    println!(
                        "New subscription request: {:?}",
                        (client_id.to_string(), &m.value().as_str().unwrap())
                    );
                    // Wait for ownership of mutex lock
                    let mut subscribers = self.subscribers.lock().unwrap();

                    // Add new subscription entry to the subscriber tree
                    subscribers.subscribe(&m.value().as_str().unwrap(), client_id.to_string());
                }
                _ => (),
            }
        };

        let subscribers = self.subscribers.lock().unwrap().get_subscribers(m.topic());

        let write_channels = subscribers.iter().map(|client_id| {
            if let Some(tx) = self.write_channel_map.lock().unwrap().get(client_id) {
                println!("Sending message to {client_id}");
                tx.clone()
            } else {
                println!("{:?}", (&subscribers, &self.write_channel_map));
                panic!("Inconsistent state between subscribers and write_channel_map");
            }
        });

        for tx in write_channels {
            tx.send(m.clone()).await.expect("Message was sent");
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
                println!(
                    "{} messages received in {}s - {} m/s",
                    count,
                    seconds,
                    (count as f64 / seconds).round()
                );
                println!("Disconnected");
                // Terminate loop
                return;
            } else {
                // println!("{:?}", message?.unwrap());
                let m: Message = message
                    .expect("message is Ok")
                    .expect("message is not None");

                // Processing messages asynchronously breaks the temporal ordering of messages, leave as synchronous for now
                // tokio::spawn(Server::on_receive(Arc::clone(&subscribers), reader.client_id().to_string(), m));
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
            println!("Connection made");
            let (r, w) = stream.into_split();

            let server_clone = self.clone();
            tokio::spawn(async move {
                _ = server_clone.receive_loop(client_id.to_string(), r).await;
            });

            let mut writer = MessageWriter::new(w);
            let (tx, rx) = mpsc::channel(32);
            let tx1 = tx.clone();
            self.write_channel_map
                .lock()
                .unwrap()
                .insert(client_id.to_string(), tx1);
            tokio::spawn(async move {
                writer.write_loop(rx).await;
            });

            let tx2 = tx.clone();
            tokio::spawn(async move {
                loop {
                    tx2.send(Message::new("test", Value::from("Ping")))
                        .await
                        .expect("Message was sent");
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                }
            });
        }
    }
}
