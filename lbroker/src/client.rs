//! A client to publish and receive messages from a server

use tokio::net::TcpStream;
use tracing::{debug, info};

use crate::config;
use crate::interface;
use rmpv::Value;

use tokio::sync::mpsc;

use futures::sink::SinkExt;
use futures::stream::StreamExt;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_util::codec::{FramedRead, FramedWrite};

use tokio;

/// A client connection to a pub/sub server.
pub struct Client {
    tx: Sender<interface::Command>,
    rx: Option<Receiver<interface::Message>>,
    addr: String,
}
impl Clone for Client {
    fn clone(&self) -> Self {
        Client {
            tx: self.tx.clone(),
            rx: None, // only one copy of receiver allowed, don't clone
            addr: self.addr.clone(),
        }
    }
}
impl Client {
    async fn send_command(&self, command: interface::Command) {
        self.tx
            .send(command)
            .await
            .expect("datagram::Command was sent")
    }

    /// Publish a message to the server
    pub async fn publish(&self, topic: &str, value: Value) {
        let message = interface::Message {
            topic: topic.to_string(),
            value,
        };
        let command = interface::Command::Publish { message };
        self.send_command(command).await;
    }

    /// Receive a message from the server
    ///
    /// Returns an interface::Message, or None when the connection has closed or timeout has
    /// been reached
    pub async fn recv(&mut self) -> Option<interface::Message> {
        if self.rx.is_some() {
            self.rx.as_mut().unwrap().recv().await
        } else {
            panic!("Cannot call recv on a clone")
        }
    }

    /// Subscribe to messages on topics matching the specified pattern
    ///
    /// Patterns can contain glob style wildcards: `*`
    pub async fn subscribe(&self, pattern: &str) {
        let command = interface::Command::Subscribe {
            pattern: pattern.to_string(),
        };
        self.send_command(command).await;
    }

    /// Unsubscribe from messages on topics matching the specified pattern
    ///
    /// See `subscribe` method for detail on patterns.
    pub async fn unsubscribe(&self, pattern: &str) {
        let command = interface::Command::Unsubscribe {
            pattern: pattern.to_string(),
        };
        self.send_command(command).await;
    }

    /// Create a new client
    ///
    /// Args:
    /// - addr: The address (`host:port`) of a server to establish a connection with
    pub async fn new(addr: String) -> Client {
        let resp = TcpStream::connect(&addr).await;
        match resp {
            Ok(stream) => {
                info!("Connected: {}", &addr);
                // Split stream into read and write halves
                let (r, w) = stream.into_split();
                let codec = interface::MsgPackCodec::<interface::Command>::new();
                let mut writer = FramedWrite::new(w, codec);
                let message_codec = interface::MsgPackCodec::<interface::Message>::new();
                let mut reader = FramedRead::new(r, message_codec);
                let (tx, rx) = mpsc::channel(config::CHANNEL_BUFFER_SIZE);
                tokio::spawn(async move {
                    while let Some(m) = reader.next().await {
                        let message = m.expect("Message was received");
                        tx.send(message)
                            .await
                            .inspect_err(|e| debug!("Error sending message over channel: {}", e))
                            .ok();
                    }
                });
                let (wtx, mut wrx) =
                    mpsc::channel::<interface::Command>(config::CHANNEL_BUFFER_SIZE);
                tokio::spawn(async move {
                    while let Some(cmd) = wrx.recv().await {
                        writer.send(cmd).await.expect("Command was sent to server");
                    }
                });
                let client = Client {
                    tx: wtx,
                    rx: Some(rx),
                    addr,
                };
                client
            }
            Err(e) => {
                panic!("{e}");
            }
        }
    }
}

/// Send some load to a server
pub async fn test_client(
    id: &str,
    address: &str,
    subscriptions: &[String],
    interval: Option<tokio::time::Interval>,
) {
    let mut client = Client::new(address.to_string()).await;
    for pattern in subscriptions {
        client.subscribe(pattern).await;
    }
    let topics = "abcdefg".chars().collect::<Vec<char>>();
    let write_future = test_client_write_loop(client.clone(), topics, interval);

    // Event handlers with message counting
    let id_clone = id.to_string();
    let read_future = tokio::spawn(async move {
        let mut msg_count = 0u64;
        let mut last_report = tokio::time::Instant::now();
        let report_interval = tokio::time::Duration::from_secs(5);

        while let Some(message) = client.recv().await {
            msg_count += 1;
            let topic = message.topic;
            let value = message.value.to_string();
            debug!("Client #{id_clone} - Message received - {topic} - {value}");

            // Periodic stats reporting
            let now = tokio::time::Instant::now();
            if now.duration_since(last_report) >= report_interval {
                let elapsed = now.duration_since(last_report).as_secs_f64();
                let rate = msg_count as f64 / elapsed;
                info!(
                    "Client #{id_clone} - Received {} msgs ({:.0} msgs/sec)",
                    msg_count, rate
                );
                msg_count = 0;
                last_report = now;
            }
        }
        panic!("Unexpectedly stopped receiving messages.")
    });

    tokio::select!(
        _ = read_future => {
            panic!("Stopped receiving messages")
        },
        _ = write_future => {
            panic!("Stopped sending messages")
        }
    );
}

/// Send some load to a server
fn test_client_write_loop(
    client: Client,
    topics: Vec<char>,
    interval: Option<tokio::time::Interval>,
) -> tokio::task::JoinHandle<()> {
    let mut interval_clone = interval;
    tokio::spawn(async move {
        let mut i = 0;
        loop {
            let topic = topics[i % topics.len()];
            client
                .publish(&topic.to_string(), Value::Boolean(true))
                .await;

            if let Some(i) = &mut interval_clone {
                i.tick().await;
            };
            i += 1;
        }
    })
}

fn test_client_read_loop(mut client: Client, id: &str) -> tokio::task::JoinHandle<()> {
    let id_clone = id.to_string();
    tokio::spawn(async move {
        while let Some(message) = client.recv().await {
            let topic = message.topic;
            let value = message.value.to_string();
            debug!("Client #{id_clone} - Message received - {topic} - {value}",);
        }
        panic!("Unexpectedly stopped receiving messages.")
    })
}
