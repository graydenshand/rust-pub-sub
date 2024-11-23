use log::{debug, info, warn};
use std::error::Error;
use uuid::Uuid;

use tokio;
use tokio::sync::broadcast;

use rmpv::Value;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::config;
use crate::glob_tree::{self};
use crate::interface::{Command, Message, MsgPackCodec};
use crate::metrics;
use futures::sink::SinkExt;
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, FramedWrite};

struct CommandProcessor {
    subscriptions: glob_tree::GlobTree,
    writer: FramedWrite<OwnedWriteHalf, MsgPackCodec<Message>>,
}
impl CommandProcessor {
    fn new(writer: FramedWrite<OwnedWriteHalf, MsgPackCodec<Message>>) -> Self {
        Self {
            subscriptions: glob_tree::GlobTree::new(),
            writer,
        }
    }

    /// Listen for messages over async channel, apply a filter, and forward over this connection
    pub async fn bind_to_channels(
        &mut self,
        mut message_channel_receiver: broadcast::Receiver<Message>,
        mut conn_channel_receiver: mpsc::Receiver<Command>,
        messages_sent: metrics::ThroughputMutator,
    ) -> Result<(), Box<dyn Error>> {
        loop {
            tokio::select! {
                message = message_channel_receiver.recv() => {
                    // Check if published message is in this client's subscriptions before sending
                    if message.is_err() {
                        debug!("{:?}", message.err());
                        continue
                    }
                    let m = message?;

                    if self.subscriptions.check(&m.topic) {
                        let f1 = messages_sent.increment();
                        let f2 = self.writer.send(m);
                        let (_,r2) = tokio::join!(f1, f2);
                        r2?
                    }
                },
                command = conn_channel_receiver.recv() => {
                    if let Some(c) = command {
                        match c {
                            Command::Subscribe { pattern } => {
                                self.subscriptions.insert(&pattern);
                            },
                            Command::Unsubscribe { pattern } => {
                                match self.subscriptions.remove(&pattern) {
                                    Ok(_) => (),
                                    Err(e) => {warn!("{}", e)}
                                };
                            },
                            _ => ()
                        }
                    }
                }
            }
        }
    }
}

struct Connection {}
impl Connection {
    fn open(
        stream: tokio::net::TcpStream,
        broadcast_sender: broadcast::Sender<Message>,
        command_throughput: metrics::ThroughputMutator,
        connection_count: metrics::CountMutator,
        messages_sent: metrics::ThroughputMutator,
    ) {
        let connection_id = Uuid::new_v4().to_string();
        info!("{} - CONNECT", connection_id);

        // A channel for the reader to forward messages directly to the writer
        let conn_channel = mpsc::channel::<Command>(config::CHANNEL_BUFFER_SIZE);

        // Split read and write halves of stream
        let (r, w) = stream.into_split();

        // Launch the loop that listens for messages from this client
        let broadcast_sender_clone = broadcast_sender.clone();
        let connection_id_clone = connection_id.clone();
        tokio::spawn(async {
            Connection::recv(
                r,
                broadcast_sender_clone,
                connection_id_clone,
                conn_channel.0,
                command_throughput,
                connection_count,
            )
            .await
            .unwrap();
        });

        // Launch the loop that listens for messages from other clients
        let message_channel_receiver = broadcast_sender.subscribe();
        tokio::spawn(async {
            Connection::send(w, message_channel_receiver, conn_channel.1, messages_sent).await;
        });
    }

    /// Receive messages from client and broadcast to rest of system
    async fn recv(
        stream: OwnedReadHalf,
        broadcast_sender: broadcast::Sender<Message>,
        connection_id: String,
        conn_channel_sender: mpsc::Sender<Command>,
        command_throughput: metrics::ThroughputMutator,
        connection_count: metrics::CountMutator,
    ) -> Result<(), Box<dyn Error>> {
        connection_count.add(1).await;
        let codec = MsgPackCodec::<Command>::new();
        let mut reader = FramedRead::new(stream, codec);
        while let Some(result) = reader.next().await {
            match result {
                Ok(command) => {
                    debug!("{} - {}", connection_id, command);
                    command_throughput.increment().await;
                    match command {
                        Command::Subscribe { pattern: _ } => {
                            conn_channel_sender.send(command.clone()).await.unwrap();
                        }
                        Command::Unsubscribe { pattern: _ } => {
                            conn_channel_sender.send(command.clone()).await.unwrap();
                        }
                        Command::Publish { message } => {
                            broadcast_sender.send(message).unwrap();
                        }
                    };
                }
                Err(e) => {
                    debug!("{connection_id} - DISCONNECT REASON - {:?}", e);
                }
            }
        }
        connection_count.subtract(1).await;
        info!("{connection_id} - DISCONNECT");
        Ok(())
    }

    /// Bind a CommandProcessor to the broadcast channel and listen for messages
    async fn send(
        stream: OwnedWriteHalf,
        receiver: broadcast::Receiver<Message>,
        conn_channel_receiver: mpsc::Receiver<Command>,
        messages_sent: metrics::ThroughputMutator,
    ) {
        tokio::spawn(async {
            let message_codec = MsgPackCodec::<Message>::new();
            let writer = FramedWrite::new(stream, message_codec);
            let mut message_processor: CommandProcessor = CommandProcessor::new(writer);
            let _ = message_processor
                .bind_to_channels(receiver, conn_channel_receiver, messages_sent)
                .await;
        });
    }
}

/// An async message passing application
#[derive(Debug)]
pub struct Server {
    /// Port on which to listen for new requests
    port: u16,
    ///  Writer to broadcast messages to all connection tasks
    broadcast_sender: broadcast::Sender<Message>,
    ///  Reader from broadcast message channel - not actually used, but the channel will close if there is not at least
    /// one active reader. To prevent the channel from closing when there are 0 connected clients, we keep a reader
    /// referenced here.
    _broadcast_receiver: broadcast::Receiver<Message>,
}
impl Server {
    /// Create a new server
    pub async fn new(port: u16) -> Result<Server, Box<dyn Error>> {
        let (broadcast_sender, _broadcast_receiver) =
            broadcast::channel(config::CHANNEL_BUFFER_SIZE);
        Ok(Server {
            port,
            broadcast_sender,
            _broadcast_receiver,
        })
    }

    fn publish_metric<T>(message_sender: &broadcast::Sender<Message>, metric: &str, value: T)
    where
        rmpv::Value: From<T>,
    {
        // Broadcast the count and calculated rate
        let m1 = Message {
            topic: format!("{}/metrics/{metric}", config::SYSTEM_TOPIC_PREFIX),
            value: Value::from(value.into()),
        };
        message_sender
            .send(m1)
            .inspect_err(|e| warn!("Failed to public metric - {metric} - {e}"))
            .ok();
    }

    /// Listen for incoming connections
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port)).await?;

        // Create metrics
        let mut command_throughput = metrics::Throughput::new();
        let command_throughput_mutator = command_throughput.get_mutator();
        let message_sender = self.broadcast_sender.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Listen for mutations from connections
                    result = command_throughput.listen() => {
                        result.unwrap()
                    }
                    // Wake up periodically to log metrics
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(config::M_COMMANDS_INTERVAL_S)) => {
                        // Broadcast the count and calculated rate
                        Self::publish_metric(&message_sender, "commands", command_throughput.value().clone());
                        Self::publish_metric(&message_sender, "commands-per-second", command_throughput.rate());

                        // Reset the metric
                        command_throughput.reset();
                    }
                }
            }
        });

        // Connections metric
        let mut connection_count = metrics::Count::new();
        let connection_count_mutator = connection_count.get_mutator();
        let message_sender = self.broadcast_sender.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = connection_count.listen() => {
                        result.unwrap()
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(config::M_CONNECTIONS_INTERVAL_S)) => {
                        Self::publish_metric(&message_sender, "connections", connection_count.value().clone());
                    }
                }
            }
        });

        // Messages sent metric
        let mut messages_sent = metrics::Throughput::new();
        let messages_sent_mutator = messages_sent.get_mutator();
        let message_sender = self.broadcast_sender.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = messages_sent.listen() => {
                        result.unwrap()
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(config::M_MESSAGES_SENT_INTERVAL_S)) => {
                        Self::publish_metric(&message_sender, "messages_sent", messages_sent.value().clone());
                        Self::publish_metric(&message_sender, "messages_sent_per_second", messages_sent.rate().clone());
                        // Reset the metric
                        messages_sent.reset();
                    }
                }
            }
        });

        loop {
            // Accept a new connection
            let (stream, _) = listener.accept().await?;

            Connection::open(
                stream,
                self.broadcast_sender.clone(),
                command_throughput_mutator.clone(),
                connection_count_mutator.clone(),
                messages_sent_mutator.clone(),
            );
        }
    }
}
