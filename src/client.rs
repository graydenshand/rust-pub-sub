use tokio::net::tcp::OwnedReadHalf;
use tokio::net::TcpStream;

use crate::config;
use crate::datagram::{Message, MessageReader, MessageWriter};
use rmpv::Value;

use log::{debug, info};
use tokio::sync::mpsc;

use std::error::Error;
use tokio::sync::mpsc::{Receiver, Sender};

use tokio;

/// A client connection to a pub/sub server.
pub struct Client {
    tx: Sender<Message>,
    rx: Option<Receiver<Message>>,
    addr: String,
}
impl Clone for Client {
    fn clone(&self) -> Self {
        Client {
            tx: self.tx.clone(),
            rx: None, // only one owner of rx allowed, don't clone
            addr: self.addr.clone(),
        }
    }
}
impl Client {
    /// Publish a message to the server
    pub async fn publish(&self, message: Message) {
        self.tx.send(message).await.expect("Message was sent")
    }

    /// Handy util for timing out an opteration that would otherwise block forever
    ///
    /// Used with tokio::select! macro, this can interrupt a process after a set
    /// duration. If no timeout is set, it will run forever.
    async fn optional_timeout(duration: Option<tokio::time::Duration>) {
        match duration {
            Some(d) => tokio::time::sleep(d).await,
            None => loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await
            },
        }
    }

    /// Receive a message from the server
    ///
    /// Returns a Message, or None when the connection has closed or timeout has
    /// been reached
    pub async fn recv(&mut self, timeout: Option<tokio::time::Duration>) -> Option<Message> {
        if self.rx.is_some() {
            tokio::select! {
                message = self.rx.as_mut().unwrap().recv() => {
                    Some(message.expect("Message can be decoded"))
                },
                _ = Self::optional_timeout(timeout) => {
                    None
                }

            }
        } else {
            panic!("Cannot call recv on a clone")
        }
    }

    async fn receive_loop(
        stream: OwnedReadHalf,
        tx: Sender<Message>,
    ) -> Result<(), Box<dyn Error>> {
        let mut reader = MessageReader::new(stream);
        loop {
            let message = reader.read_value().await?;
            if let Some(m) = message {
                tx.send(m).await?;
            } else {
                return Ok(());
            }
        }
    }

    /// Subscribe to messages on topics matching the specified pattern
    ///
    /// Patterns can contain glob style wildcards: `*``
    ///
    /// System messages are sent on topics starting with `!` (e.g. `!subscriptions`),
    /// and are excluded in wildcard matches. To subscribe to all system messages,
    /// create a new subscription with the following pattern: `!`
    pub async fn subscribe(&self, pattern: &str) {
        let topic = format!("{}{}", config::SYSTEM_TOPIC_PREFIX, config::SUBSCRIBE_TOPIC);
        self.publish(Message::new(&topic, Value::from(pattern)))
            .await;
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
                let (r, w) = stream.into_split();
                let (tx, rx) = mpsc::channel(config::CHANNEL_BUFFER_SIZE);
                tokio::spawn(async move {
                    _ = Client::receive_loop(r, tx).await.unwrap();
                });
                let (wtx, wrx) = mpsc::channel(config::CHANNEL_BUFFER_SIZE);
                let mut writer = MessageWriter::new(w);
                tokio::spawn(async move {
                    writer.subscribe_to_channel(wrx).await.ok();
                });
                Client {
                    tx: wtx,
                    rx: Some(rx),
                    addr,
                }
            }
            Err(e) => {
                panic!("{e}");
            }
        }
    }
}
