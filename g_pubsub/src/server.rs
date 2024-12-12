//! A TCP pub/sub server

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
use crate::interface::{Command, Message, MsgPackCodec};
use crate::metrics;
use crate::metrics::Metric;
use futures::sink::SinkExt;
use glob_tree::{self};
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
        metrics_mutator: MetricsMutator,
    ) -> Result<(), Box<dyn Error>> {
        loop {
            tokio::select! {
                message = message_channel_receiver.recv() => {
                    if message.is_err() {
                        debug!("{}", message.err().unwrap());
                        continue
                    }
                    let m = message?;

                    // Check if published message is in this client's subscriptions before sending
                    match self.subscriptions.check(&m.topic) {
                        None => (),
                        Some(pattern) => {
                            // Suppress system topics unless explicitly subscribed
                            if !(m.topic.starts_with(config::SYSTEM_TOPIC_PREFIX) && !pattern.starts_with(config::SYSTEM_TOPIC_PREFIX)) {
                                let (_,r2) = tokio::join!(
                                    metrics_mutator.messages_sent.increment(),
                                    self.writer.send(m)
                                );
                                r2?
                            }
                        }
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
        metrics_mutator: MetricsMutator,
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
        let metrics_mutator_clone = metrics_mutator.clone();
        tokio::spawn(async {
            Connection::recv(
                r,
                broadcast_sender_clone,
                connection_id_clone,
                conn_channel.0,
                metrics_mutator_clone,
            )
            .await
            .unwrap();
        });

        // Launch the loop that listens for messages from other clients
        let message_channel_receiver = broadcast_sender.subscribe();
        tokio::spawn(async {
            Connection::send(w, message_channel_receiver, conn_channel.1, metrics_mutator).await;
        });
    }

    /// Receive messages from client and broadcast to rest of system
    async fn recv(
        stream: OwnedReadHalf,
        broadcast_sender: broadcast::Sender<Message>,
        connection_id: String,
        conn_channel_sender: mpsc::Sender<Command>,
        metrics_mutator: MetricsMutator,
    ) -> Result<(), Box<dyn Error>> {
        metrics_mutator.connection_count.add(1).await;
        let codec = MsgPackCodec::<Command>::new();
        let mut reader = FramedRead::new(stream, codec);
        while let Some(result) = reader.next().await {
            match result {
                Ok(command) => {
                    debug!("{} - {}", connection_id, command);
                    metrics_mutator.command_throughput.increment().await;
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
        metrics_mutator.connection_count.subtract(1).await;
        info!("{connection_id} - DISCONNECT");
        Ok(())
    }

    /// Bind a CommandProcessor to the broadcast channel and listen for messages
    async fn send(
        stream: OwnedWriteHalf,
        receiver: broadcast::Receiver<Message>,
        conn_channel_receiver: mpsc::Receiver<Command>,
        metrics_mutator: MetricsMutator,
    ) {
        tokio::spawn(async {
            let message_codec = MsgPackCodec::<Message>::new();
            let writer = FramedWrite::new(stream, message_codec);
            let mut message_processor: CommandProcessor = CommandProcessor::new(writer);
            let _ = message_processor
                .bind_to_channels(receiver, conn_channel_receiver, metrics_mutator)
                .await;
        });
    }
}

#[derive(Clone)]
struct MetricsMutator {
    /// Metrics
    command_throughput: metrics::ThroughputMutator,
    connection_count: metrics::CountMutator,
    messages_sent: metrics::ThroughputMutator,
}

/// An async message passing application
#[derive(Debug)]
pub struct Server {
    /// Port on which to listen for new requests
    port: u16,
    ///  Writer to broadcast messages to all connection tasks
    broadcast_sender: broadcast::Sender<Message>,
    /// Reader from broadcast message channel - not actually used, but the channel will close if there is not at least
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

    fn register_metrics(&self) -> MetricsMutator {
        // Command throughput
        let mut command_throughput = metrics::Throughput::new(
            "commands",
            tokio::time::Duration::from_secs(config::M_COMMANDS_INTERVAL_S),
        );
        let command_throughput_mutator = command_throughput.get_mutator();
        let message_sender = self.broadcast_sender.clone();
        tokio::spawn(async move {
            command_throughput
                .listen_and_broadcast(message_sender)
                .await;
        });

        // Connections metric
        let mut connection_count = metrics::Count::new(
            "connections",
            tokio::time::Duration::from_secs(config::M_CONNECTIONS_INTERVAL_S),
        );
        let connection_count_mutator = connection_count.get_mutator();
        let message_sender = self.broadcast_sender.clone();
        tokio::spawn(async move {
            connection_count.listen_and_broadcast(message_sender).await;
        });

        // Messages sent metric
        let mut messages_sent = metrics::Throughput::new(
            "messages-sent",
            tokio::time::Duration::from_secs(config::M_MESSAGES_SENT_INTERVAL_S),
        );
        let message_sender = self.broadcast_sender.clone();
        let messages_sent_mutator = messages_sent.get_mutator();
        tokio::spawn(async move {
            messages_sent.listen_and_broadcast(message_sender).await;
        });

        MetricsMutator {
            command_throughput: command_throughput_mutator,
            connection_count: connection_count_mutator,
            messages_sent: messages_sent_mutator,
        }
    }

    /// Listen for incoming connections
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port)).await?;

        let metrics_mutator = self.register_metrics();

        loop {
            // Accept a new connection
            let (stream, _) = listener.accept().await?;

            Connection::open(
                stream,
                self.broadcast_sender.clone(),
                metrics_mutator.clone(),
            );
        }
    }
}
