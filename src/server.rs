use log::{debug, info, warn};
use std::error::Error;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use tokio;
use tokio::sync::broadcast;

use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::config;
use crate::datagram::{bind_stream, send_rmp_value, Command, Datagram, Message};
use crate::glob_tree::{self};

#[derive(Debug)]
struct Channel {
    tx: broadcast::Sender<Message>,
    rx: broadcast::Receiver<Message>,
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
    stream: OwnedWriteHalf,
}
impl DatagramProcessor {
    fn new(stream: OwnedWriteHalf) -> Self {
        Self {
            subscriptions: glob_tree::GlobTree::new(),
            stream,
        }
    }

    /// Listen for messages over async channel, apply a filter, and forward over this connection
    pub async fn bind_to_channels(
        &mut self,
        mut message_channel_receiver: broadcast::Receiver<Message>,
        mut conn_channel_receiver: mpsc::Receiver<Command>,
    ) -> Result<(), Box<dyn Error>> {
        loop {
            tokio::select! {
                message = message_channel_receiver.recv() => {
                    // Check if published message is in this client's subscriptions before sending
                    let m = message.expect("Received message from channel");
                    if self.subscriptions.check(&m.topic) {
                        send_rmp_value(&mut self.stream, m).await?;
                    }
                },
                command = conn_channel_receiver.recv() => {
                    if let Some(c) = command {
                        match c {
                            Command::Subscribe { pattern } => {
                                self.subscriptions.insert(&pattern);
                            },
                            _ => ()
                        }
                    }
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
        let client_id = Uuid::new_v4().to_string();
        info!("{} - CONNECT", client_id);

        // A channel for the reader to forward messages directly to the writer
        let conn_channel = mpsc::channel::<Command>(config::CHANNEL_BUFFER_SIZE);

        // Split read and write halves of stream
        let (r, w) = stream.into_split();

        // Launch the loop that listens for messages from this client
        let message_channel_sender = channel.tx.clone();
        let client_id_clone = client_id.clone();
        tokio::spawn(async {
            Connection::recv(r, message_channel_sender, client_id_clone, conn_channel.0)
                .await
                .unwrap();
        });

        // Launch the loop that listens for messages from other clients
        let message_channel_receiver = channel.tx.subscribe();
        tokio::spawn(async {
            Connection::send(w, message_channel_receiver, conn_channel.1).await;
        });
    }

    /// Receive messages from client and broadcast to rest of system
    async fn recv(
        stream: OwnedReadHalf,
        message_channel_sender: broadcast::Sender<Message>,
        client_id: String,
        conn_channel_sender: mpsc::Sender<Command>,
    ) -> Result<(), Box<dyn Error>> {
        let r = bind_stream(stream, |datagram: Datagram| async {
            debug!("{} - {}", client_id, datagram.command);
            match datagram.command {
                Command::Subscribe { pattern: _ } => {
                    conn_channel_sender
                        .send(datagram.command.clone())
                        .await
                        .unwrap();
                }
                Command::Publish { message } => {
                    message_channel_sender.send(message).unwrap();
                }
            };
        })
        .await;
        if r.is_err() {
            debug!("{} - {:?}", client_id, r.err());
        }
        info!("{} - DISCONNECT", client_id);
        Ok(())
    }

    /// Bind a DatagramProcessor to the broadcast channel and listen for messages
    async fn send(
        stream: OwnedWriteHalf,
        rx: broadcast::Receiver<Message>,
        conn_channel_receiver: mpsc::Receiver<Command>,
    ) {
        tokio::spawn(async {
            let mut message_processor: DatagramProcessor = DatagramProcessor::new(stream);
            message_processor
                .bind_to_channels(rx, conn_channel_receiver)
                .await
                .expect("Processes messages from channel");
        });
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
