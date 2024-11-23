use log::{debug, info};
use tokio::net::TcpStream;

use crate::config;
use crate::datagram;
use rmpv::Value;

use tokio::sync::mpsc;

use futures::sink::SinkExt;
use futures::stream::StreamExt;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_util::codec::{FramedRead, FramedWrite};

use tokio;

/// A client connection to a pub/sub server.
pub struct Client {
    tx: Sender<datagram::Command>,
    rx: Option<Receiver<datagram::Message>>,
    addr: String,
    client_id: String,
}
impl Clone for Client {
    fn clone(&self) -> Self {
        Client {
            tx: self.tx.clone(),
            rx: None, // only one copy of receiver allowed, don't clone
            addr: self.addr.clone(),
            client_id: self.client_id.clone(),
        }
    }
}
impl Client {
    async fn send_command(&self, command: datagram::Command) {
        self.tx
            .send(command)
            .await
            .expect("datagram::Command was sent")
    }

    /// Publish a message to the server
    pub async fn publish(&self, topic: &str, value: Value) {
        let message = datagram::Message {
            topic: topic.to_string(),
            value,
        };
        let command = datagram::Command::Publish { message };
        self.send_command(command).await;
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
    /// Returns a datagram::Command, or None when the connection has closed or timeout has
    /// been reached
    pub async fn recv(
        &mut self,
        timeout: Option<tokio::time::Duration>,
    ) -> Option<datagram::Message> {
        if self.rx.is_some() {
            tokio::select! {
                message = self.rx.as_mut().unwrap().recv() => {
                    message
                },
                _ = Self::optional_timeout(timeout) => {
                    None
                }

            }
        } else {
            panic!("Cannot call recv on a clone")
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
        let command = datagram::Command::Subscribe {
            pattern: pattern.to_string(),
        };
        self.send_command(command).await;
    }

    /// Unsubscribe from messages on topics matching the specified pattern
    ///
    /// See `subscribe` method for detail on patterns.
    pub async fn unsubscribe(&self, pattern: &str) {
        let command = datagram::Command::Unsubscribe {
            pattern: pattern.to_string(),
        };
        self.send_command(command).await;
    }

    /// Create a new client
    ///
    /// Args:
    /// - addr: The address (`host:port`) of a server to establish a connection with
    /// - client_id: A unique identifier for this connection
    pub async fn new(addr: String, client_id: String) -> Client {
        let resp = TcpStream::connect(&addr).await;
        match resp {
            Ok(stream) => {
                info!("Connected: {}", &addr);
                // Split stream into read and write halves
                let (r, w) = stream.into_split();
                let datagram_codec = datagram::MsgPackCodec::<datagram::Datagram>::new();
                let mut writer = FramedWrite::new(w, datagram_codec);
                let message_codec = datagram::MsgPackCodec::<datagram::Message>::new();
                let mut reader = FramedRead::new(r, message_codec);
                let (tx, rx) = mpsc::channel(config::CHANNEL_BUFFER_SIZE);
                tokio::spawn(async move {
                    while let Some(m) = reader.next().await {
                        match m {
                            Ok(message) => tx
                                .send(message)
                                .await
                                .expect("Message failed to send over channel"),
                            Err(e) => panic!("Error {}", e),
                        }
                    }
                });
                let (wtx, mut wrx) =
                    mpsc::channel::<datagram::Command>(config::CHANNEL_BUFFER_SIZE);
                let client_id_clone = client_id.clone();
                tokio::spawn(async move {
                    while let Some(cmd) = wrx.recv().await {
                        let datagram = datagram::Datagram::new(cmd, &client_id_clone);
                        writer
                            .send(datagram)
                            .await
                            .expect("Command was sent to server");
                    }
                });
                let client = Client {
                    tx: wtx,
                    rx: Some(rx),
                    addr,
                    client_id,
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
pub async fn test_client(address: &str, client_id: &str) {
    let mut client = Client::new(address.to_string(), client_id.to_string()).await;
    // client.subscribe("*").await;

    let client_clone = client.clone();
    let write_future = tokio::spawn(async move {
        let mut i = 0;
        loop {
            client_clone.publish("test", Value::from(i)).await;
            i += 1;
        }
    });

    // Event handlers
    let read_future = tokio::spawn(async move {
        let mut i = 0;
        while let Some(message) = client.recv(None).await {
            if i % 10_000 == 0 {
                let topic = message.topic;
                let value = message.value.to_string();
                debug!("Message received - {topic} - {value}");
            };
            i += 1;
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
