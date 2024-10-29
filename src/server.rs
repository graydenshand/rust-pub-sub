
use log::{debug, info, warn};
use std::error::Error;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use tokio;
use tokio::sync::broadcast;

use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpListener;
use tokio::time::Instant;

use crate::config::{self, SYSTEM_TOPIC_PREFIX};
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

enum Command {
    Publish,
    Subscribe,
    SetClientId,
}
impl Command {
    fn from_topic(topic: &str) -> Result<Command, Box<dyn Error>> {
        if !topic.starts_with(config::SYSTEM_TOPIC_PREFIX) { return Ok(Command::Publish)};
        match topic.trim_start_matches(config::SYSTEM_TOPIC_PREFIX) {
            config::SUBSCRIBE_TOPIC => Ok(Command::Subscribe),
            config::SET_CLIENT_ID_TOPIC => Ok(Command::SetClientId),
            _ => Err("Unrecognized system topic".into()),
        }
    }
}

struct MessageProcessor {
    subscribers: subscription_tree::SubscriptionTree,
    writer: MessageWriter,
    client_id: String
}
impl MessageProcessor {

    fn new(stream: OwnedWriteHalf, client_id: String) -> Self {
        Self {
            subscribers: subscription_tree::SubscriptionTree::new(),
            writer: MessageWriter::new(stream),
            client_id,
        }
    }

    /// Process a message
    /// 
    /// Returns true if the message should get sent over connection
    async fn process_message(&mut self, message: &Message) ->  bool {
        // Handle system messages
        let command = Command::from_topic(message.topic());
        if command.is_ok() {
            match command.unwrap() {
                Command::Subscribe => {
                    if message.client_id() == self.client_id {
                        self.subscribers.subscribe(&message.value().as_str().unwrap());
                    }
                },
                Command::SetClientId => {
                    self.client_id = message.value().as_str().unwrap().to_string();
                },
                _ => (),
            
            }
        }
        self.subscribers.is_subscribed(message.topic())
    }

    /// Listen for messages over async channel, apply a filter, and forward over this connection
    pub async fn bind_to_channel(
        &mut self,
        mut rx: broadcast::Receiver<Message>,
    ) -> Result<(), Box<dyn Error>> {
        while let Some(message) = rx.recv().await.ok() {
            if self.process_message(&message).await {
                self.writer.send(message).await?
            }
        }
        Ok(())
    }
}

struct Connection {}
impl Connection {
    fn open(stream: tokio::net::TcpStream, channel: Channel) {
        let client_id = Uuid::new_v4().to_string();
        info!("{} - CONNECT", client_id);

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
    async fn recv(stream: OwnedReadHalf, tx: broadcast::Sender<Message>, mut client_id: String) {
        let mut reader = MessageReader::new(stream);
        let start = Instant::now();
        let mut count = 0;
        reader.bind(|m| {
            match Command::from_topic(m.topic()) {
                Ok(command) => {
                    match command {
                        Command::Publish => {
                            debug!("{client_id} - PUBLISH - {} {}", m.topic(), m.value());
                        },
                        Command::Subscribe => {
                            debug!("{client_id} - SUBSCRIBE - {}", m.value().as_str().unwrap())
                        }
                        Command::SetClientId => {
                            debug!("{client_id} - SET CLIENT ID - {}", m.value().as_str().unwrap());
                            client_id = m.value().as_str().unwrap().to_string();
                        }
                    }
                },
                Err(e) => {
                    warn!("{}", e);
                }
            }
            tx.send(m).unwrap();
            count += 1;
        }).await.unwrap();
        let end = Instant::now();
        let seconds = (end - start).as_millis() as f64 / 1000.0;
        info!(
            "{client_id} - DISCONNECT - {count} messages received in {seconds}s - {} m/s",
            (count as f64 / seconds).round()
        );
    }

    /// Bind a MessageProcessor to the broadcast channel and listen for messages
    async fn send(stream: OwnedWriteHalf, rx: broadcast::Receiver<Message>, client_id: String) {
        tokio::spawn(async {
            let mut message_processor: MessageProcessor = MessageProcessor::new(stream, client_id);
            message_processor.bind_to_channel(rx).await.ok();
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
