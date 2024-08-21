use rmpv::Value;

use std::alloc::System;
use std::error::Error;
use std::time::{Instant};
use chrono::DateTime;
use std::sync::{Arc, Mutex};

use tokio;

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
#[derive(Debug)]
pub struct Server {
    /// Port on which to listen for new requests
    port: u16,
    /// Structure containing information about clients subscribed to topics on this Server
    subscribers: Arc<Mutex<subscription_tree::SubscriptionTree<String>>>,
}
impl Server {
    pub async fn new(port: u16) -> Result<Server, Box<dyn Error>> {
        Ok(Server {
            port,
            subscribers: Arc::new(Mutex::new(subscription_tree::SubscriptionTree::new())),
        })
    }

    /// Process a message published by a client
    fn on_receive(subscribers: Arc<Mutex<subscription_tree::SubscriptionTree<String>>>, client_id: String, m: Message) {
        println!("{:?} Message received - {client_id} - {} - {}", chrono::offset::Local::now(), m.topic(), m.value());
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
                    let mut subscribers = subscribers.lock().unwrap();

                    // Add new subscription entry to the subscriber tree
                    subscribers.subscribe(&m.value().as_str().unwrap(), client_id);
                }
                _ => (),
            }
        };
        for client_id in subscribers.lock().unwrap().get_subscribers(m.topic()) {
            println!("Sending message to {client_id}");
        }
    }

    /// Maintain connection with a client and handle published messages
    pub async fn receive_loop(subscribers: Arc<Mutex<subscription_tree::SubscriptionTree<String>>>, stream: OwnedReadHalf) -> Result<(), Box<dyn Error>> {
        let mut reader = MessageReader::new(stream);
        let start = Instant::now();

        let mut count = 0;

        loop {
            let message = reader.read_value().await;
            if message.is_err() || message.as_ref().unwrap().is_none() {
                // Unsubscribe client from all topics
                let _ = subscribers.lock().unwrap().unsubscribe_client(reader.client_id().to_string());

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
                return Ok(());
            } else {
                // println!("{:?}", message?.unwrap());
                let m: Message = message.expect("message is Ok").expect("message is not None");

                // Processing messages asynchronously breaks the temporal ordering of messages, leave as synchronous for now
                // tokio::spawn(Server::on_receive(Arc::clone(&subscribers), reader.client_id().to_string(), m));
                Server::on_receive(Arc::clone(&subscribers), reader.client_id().to_string(), m);
            }

            count += 1;
        }
    }

    /// Listen for incoming connections
    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port)).await?;

        loop {
            let (stream, _) = listener.accept().await?;
            println!("Connection made");
            let (r, w) = stream.into_split();
            let subscribers = Arc::clone(&self.subscribers);
            tokio::spawn(async move {
                _ = Server::receive_loop(subscribers, r).await;
            });

            // let mut writer = MessageWriter::new(w);
            // tokio::spawn(async move {
            //     _ = writer
            //         .send(Message::new("test", Value::Boolean(true)))
            //         .await;
            // });
        }
    }
}
