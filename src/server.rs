use log::{debug, info, warn};
use std::error::Error;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use tokio;
use tokio::sync::broadcast;

use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpListener;

use crate::config;
use crate::datagram::{bind_stream, send_rmp_value, Command, Datagram};
use crate::glob_tree::{self};

#[derive(Debug)]
struct Channel {
    tx: broadcast::Sender<Datagram>,
    rx: broadcast::Receiver<Datagram>,
}
impl Channel {
    fn new(capacity: usize) -> Self {
        let (tx, rx) = broadcast::channel(capacity);
        Self { tx, rx }
    }
}
impl Clone for Channel {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            rx: self.tx.subscribe(),
        }
    }
}

struct DatagramProcessor {
    subscriptions: glob_tree::GlobTree,
    client_id: Arc<Mutex<String>>,
    stream: OwnedWriteHalf,
}
impl DatagramProcessor {
    fn new(stream: OwnedWriteHalf, client_id: Arc<Mutex<String>>) -> Self {
        Self {
            subscriptions: glob_tree::GlobTree::new(),
            stream,
            client_id,
        }
    }

    /// Listen for messages over async channel, apply a filter, and forward over this connection
    pub async fn bind_to_channel(
        &mut self,
        mut rx: broadcast::Receiver<Datagram>,
    ) -> Result<(), Box<dyn Error>> {
        loop {
            let datagram = rx.recv().await;
            match datagram {
                Ok(d) => {
                    match &d.command {
                        Command::Subscribe { pattern } => {
                            // Add a new subscription for this client
                            self.subscriptions.insert(pattern);
                        }
                        Command::Publish { message } => {
                            // Check if published message is in this client's subscriptions before sending
                            if self.subscriptions.check(&message.topic) {
                                send_rmp_value(&mut self.stream, message).await.unwrap();
                            }
                        }
                        _ => (),
                    }
                }
                Err(e) => {
                    warn!("{e}");
                    continue;
                }
            }
        }
    }
}

struct Connection {
    client_id: Arc<Mutex<String>>,
}
impl Connection {
    fn open(stream: tokio::net::TcpStream, channel: Channel) {
        let client_id = Arc::new(Mutex::new(Uuid::new_v4().to_string()));
        info!("{} - CONNECT", client_id.lock().unwrap());

        // Split read and write halves of stream
        let (r, w) = stream.into_split();

        // Launch the loop that listens for messages from this client
        let tx = channel.tx.clone();
        let client_id_clone = client_id.clone();
        tokio::spawn(async {
            Connection::recv(r, tx, client_id_clone).await;
        });

        // Launch the loop that listens for messages from other clients
        let rx = channel.tx.subscribe();
        tokio::spawn(async {
            Connection::send(w, rx, client_id).await;
        });
    }

    /// Receive messages from client and broadcast to rest of system
    async fn recv(
        stream: OwnedReadHalf,
        tx: broadcast::Sender<Datagram>,
        client_id: Arc<Mutex<String>>,
    ) {
        bind_stream(stream, |datagram: Datagram| async {
            {
                let c = client_id.lock().unwrap();
                // debug!("{} - {}", c, datagram.command);
            }
            match &datagram.command {
                Command::SetClientId { id } => {
                    *client_id.lock().unwrap() = id.to_string();
                }
                _ => (),
            };
            tx.send(datagram).unwrap();
        })
        .await
        .unwrap();
        info!("{} - DISCONNECT", client_id.lock().unwrap());
    }

    /// Bind a DatagramProcessor to the broadcast channel and listen for messages
    async fn send(
        stream: OwnedWriteHalf,
        rx: broadcast::Receiver<Datagram>,
        client_id: Arc<Mutex<String>>,
    ) {
        tokio::spawn(async {
            let mut message_processor: DatagramProcessor =
                DatagramProcessor::new(stream, client_id);
            message_processor
                .bind_to_channel(rx)
                .await
                .expect("Processes messages from channel");
        })
        .await
        .expect("Message processor failed");
    }
}

/// An async message passing application
#[derive(Debug, Clone)]
pub struct Server {
    /// Port on which to listen for new requests
    port: u16,
    ///  Broadcast communication channel for all connection tasks
    channel: Channel,
}
impl Server {
    /// Create a new server
    pub async fn new(port: u16) -> Result<Server, Box<dyn Error>> {
        Ok(Server {
            port,
            channel: Channel::new(config::CHANNEL_BUFFER_SIZE),
        })
    }

    /// Listen for incoming connections
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port)).await?;

        loop {
            // Accept a new connection
            let (stream, _) = listener.accept().await?;

            Connection::open(stream, self.channel.clone());
        }
    }
}
