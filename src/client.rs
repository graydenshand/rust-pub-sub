use log::info;
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::TcpStream;

use crate::config;
use crate::datagram;
use rmpv::Value;

use tokio::sync::mpsc;

use std::error::Error;
use tokio::sync::mpsc::{Receiver, Sender};

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

    async fn receive_loop(
        stream: OwnedReadHalf,
        tx: Sender<datagram::Message>,
    ) -> Result<(), Box<dyn Error>> {
        datagram::bind_stream(stream, |message: datagram::Message| async {
            tx.send(message).await.expect("Ok")
        })
        .await
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

    async fn set_client_id(&self) {
        let command = datagram::Command::SetClientId {
            id: self.client_id.to_string(),
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
                let (r, mut w) = stream.into_split();
                let (tx, rx) = mpsc::channel(config::CHANNEL_BUFFER_SIZE);
                tokio::spawn(async move {
                    _ = Client::receive_loop(r, tx).await.unwrap();
                });
                let (wtx, mut wrx) =
                    mpsc::channel::<datagram::Command>(config::CHANNEL_BUFFER_SIZE);
                let client_id_clone = client_id.clone();
                tokio::spawn(async move {
                    while let Some(cmd) = wrx.recv().await {
                        let datagram = datagram::Datagram::new(cmd, &client_id_clone);
                        datagram::send_rmp_value(&mut w, datagram).await.unwrap();
                    }
                });
                let client = Client {
                    tx: wtx,
                    rx: Some(rx),
                    addr,
                    client_id,
                };
                client.set_client_id().await;
                client
            }
            Err(e) => {
                panic!("{e}");
            }
        }
    }
}
