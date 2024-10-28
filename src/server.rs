
use log::{debug, info, warn};
use std::error::Error;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use tokio;
use tokio::sync::broadcast;

use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpListener;

use crate::config;
use crate::datagram::{Message, MessageReader, MessageWriter};
use crate::subscription_tree::{self};


#[derive(Debug)]
struct Channel {
    tx: broadcast::Sender<Message>,
    rx: broadcast::Receiver<Message>,
}
impl Channel {
    fn new(capacity: usize) -> Self {
        let (tx, rx) = broadcast::channel(capacity);
        Self {
            tx,
            rx
        }
    }
}
impl Clone for Channel {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            rx: self.tx.subscribe()
        }
    }
}

struct ConnectionProcessor {
    subscribers: subscription_tree::SubscriptionTree,
    writer: MessageWriter,
}
impl ConnectionProcessor {

    fn new(stream: OwnedWriteHalf) -> Self {
        Self {
            subscribers: subscription_tree::SubscriptionTree::new(),
            writer: MessageWriter::new(stream)
        }
    }

    /// Process system messages
    fn handle_system_message(&mut self, message: &Message) {
        match message
            .topic()
            .trim_start_matches(config::SYSTEM_TOPIC_PREFIX)
        {
            config::SUBSCRIBE_TOPIC => {
                // Message value contains subscription pattern
                debug!(
                    "SUBSCRIBE - {}",
                    &message.value().as_str().unwrap()
                );

                // Add new subscription entry to the subscriber tree
                self.subscribers.subscribe(&message.value().as_str().unwrap());
            },
            config::HEALTH_TOPIC => {
                // No Op
            }
            _ => {
                warn!("Message published to unrecognized system topic: {}", message.topic());
            }
        }
    }

    /// Handle a message
    /// 
    /// Returns true if the message should get sent over connection
    async fn on_receive(&mut self, message: &Message) ->  bool {
        let topic = message.topic();
        let value = message.value();
        
        // Handle system messages
        if message.topic().starts_with(config::SYSTEM_TOPIC_PREFIX) {
            self.handle_system_message(&message);
        } else {
            debug!("PUBLISH - {topic} {value}");
        };

        self.subscribers.is_subscribed(message.topic())
    }

    /// Listen for messages over async channel, apply a filter, and forward over this connection
    pub async fn subscribe_to_channel(
        &mut self,
        mut rx: broadcast::Receiver<Message>,
    ) -> Result<(), Box<dyn Error>> {
        while let Some(message) = rx.recv().await.ok() {
            if self.on_receive(&message).await {
                self.writer.send(message).await?
            }
        }
        Ok(())
    }
}

struct Connection {}
impl Connection {
    fn new(stream: tokio::net::TcpStream, channel: Channel) {
        // Assign a client id to this connection
        let client_id = Uuid::new_v4();
        info!("CONNECT - {}", client_id.to_string());

        // Split read and write halves of stream
        let (r, w) = stream.into_split();

        // Launch the loop that listens for messages from this client
        let tx = channel.tx.clone();
        tokio::spawn(async move {
            Connection::recv(r, tx).await;
        });

        // Launch the loop that listens for messages from other clients
        let rx = channel.tx.subscribe();
        tokio::spawn(async move {
            Connection::send(w, rx).await;
        });
    }

    /// Receive messages from client and broadcast to rest of system
    async fn recv(stream: OwnedReadHalf, tx: broadcast::Sender<Message>) {
        let mut reader = MessageReader::new(stream);
        reader.subscribe_to_channel(tx).await.ok();
    }

    async fn send(stream: OwnedWriteHalf, rx: broadcast::Receiver<Message>) {
        // Bind a ConnectionProcessor to the broadcast channel and listen for messages
        let mut connection_processor = ConnectionProcessor::new(stream);
        tokio::spawn(async move {
            connection_processor.subscribe_to_channel(rx).await.ok();
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

            Connection::new(stream, self.channel.clone());
        }
    }
}
