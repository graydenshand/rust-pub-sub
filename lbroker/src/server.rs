//! A TCP pub/sub server

use log::{debug, info, warn};
use std::error::Error;

use tokio;
use tokio::sync::broadcast;

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

/// A message bus for passing messages between connections
///
/// The bus is a thin wrapper around a tokio broadcast channel. Every clone of the bus is connected - a message sent
/// on one clone of the bus is instantly available to receive from all other clones.
#[derive(Debug)]
struct Bus {
    receiver: broadcast::Receiver<Message>,
    sender: broadcast::Sender<Message>,
}
impl Bus {
    pub fn new() -> Self {
        let (sender, receiver) = broadcast::channel(config::CHANNEL_BUFFER_SIZE);
        Self { sender, receiver }
    }

    pub fn send(&self, message: Message) {
        self.sender
            .send(message)
            .inspect_err(|e| debug!("Error sending message to bus - {e}"))
            .ok();
    }

    pub async fn recv(&mut self) {
        self.receiver
            .recv()
            .await
            .inspect_err(|e| debug!("Error receiving message from bus - {e}"))
            .ok();
    }

    pub fn into_channels(self) -> (broadcast::Sender<Message>, broadcast::Receiver<Message>) {
        (self.sender, self.receiver)
    }
}
impl Clone for Bus {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            receiver: self.sender.subscribe(),
        }
    }
}

/// An interface that supports receiving commands and sending messages to a client.
trait Adapter {
    /// Consume this adapter and convert it into independent sender and receiver channels
    fn into_channels(self) -> (mpsc::Sender<Message>, mpsc::Receiver<Command>);
}

/// Spawn a new connection
///
/// The connection bridges an adapter and the bus. It consists of a subscription manager which is responsible for
/// managing a client's subscriptions and filtering the message stream accordingly, and a command router which relays
/// commands either to the bus or to the subscription manager.
async fn spawn_connection<T: Adapter>(adapter: T, bus: Bus, metrics: MetricsMutator) {
    let (adapter_sender, adapter_receiver) = adapter.into_channels();
    let (bus_sender, bus_receiver) = bus.into_channels();

    tokio::spawn(async move {
        metrics.connection_count.add(1).await;
        let sub_manager_sender =
            spawn_subscription_manager(adapter_sender, bus_receiver, metrics.clone());
        spawn_command_router(
            adapter_receiver,
            bus_sender,
            sub_manager_sender,
            metrics.clone(),
        )
        .await
        .inspect_err(|e| warn!("command router exited with an error - {e}"))
        .ok();
        metrics.connection_count.subtract(1).await;
    });
}

/// Spawn a command router.
///
/// The command router routes incoming commands from an adapter. The messages
/// contained in Publish commands are broadcast to the bus. Subscribe
/// and Unsubscribe commands are sent directly to the subscription manager
/// for this connection.
fn spawn_command_router(
    mut adapter_receiver: mpsc::Receiver<Command>,
    bus_sender: broadcast::Sender<Message>,
    sub_manager_sender: mpsc::Sender<Command>,
    metrics: MetricsMutator,
) -> tokio::task::JoinHandle<()> {
    // spawn command processing task
    tokio::spawn(async move {
        while let Some(command) = adapter_receiver.recv().await {
            metrics.command_throughput.increment().await;
            match command {
                Command::Subscribe { pattern: _ } | Command::Unsubscribe { pattern: _ } => {
                    sub_manager_sender.send(command.clone()).await.unwrap();
                }
                Command::Publish { message } => {
                    bus_sender
                        .send(message)
                        .inspect_err(|e| warn!("Failed to send message to bus - {e}"))
                        .ok();
                }
            };
        }
    })
}

/// Spawn a subscription manager.
///
/// The subscription manager receives messages from the bus, forwarding them to an adapter. It owns the set of topic
/// patterns the connection is subscribed to.
///
/// Returns a channel for sending commands (e.g. 'Subscribe', and 'Unsusbscribe') to this subscription manager.
fn spawn_subscription_manager(
    adapter_sender: mpsc::Sender<Message>,
    mut bus_receiver: broadcast::Receiver<Message>,
    metrics: MetricsMutator,
) -> mpsc::Sender<Command> {
    let (sub_manager_sender, mut sub_manager_receiver) = mpsc::channel(config::CHANNEL_BUFFER_SIZE);
    tokio::spawn(async move {
        let mut subscriptions = glob_tree::GlobTree::new();
        loop {
            // Use select to concurrently receive from two channels and yield the first value from either channel
            tokio::select! {
                // Receive message from the message bus
                message = bus_receiver.recv() => {
                    if message.is_err() {
                        debug!("{}", message.err().unwrap());
                        continue
                    }
                    let m = message.unwrap();

                    // Check if published message is in this client's subscriptions before sending
                    match subscriptions.check(&m.topic) {
                        None => (),
                        Some(pattern) => {
                            // Suppress system topics unless explicitly subscribed
                            if !(m.topic.starts_with(config::SYSTEM_TOPIC_PREFIX) && !pattern.starts_with(config::SYSTEM_TOPIC_PREFIX)) {
                                // Increment messages_sent metric
                                metrics.messages_sent.increment().await;

                                // Send the message through the adapter, if an error occurs the adapter is closed - exit
                                match adapter_sender.send(m).await {
                                    Ok(_) => (),
                                    Err(_) => return
                                };
                            }
                        }
                    }
                },
                // Receive commands directly from the read side of this connection
                command = sub_manager_receiver.recv() => {
                    if let Some(c) = command {
                        match c {
                            Command::Subscribe { pattern } => {
                                subscriptions.insert(&pattern);
                            },
                            Command::Unsubscribe { pattern } => {
                                match subscriptions.remove(&pattern) {
                                    Ok(_) => (),
                                    Err(e) => {warn!("{}", e)}
                                };
                            },
                            Command::Publish { .. } => ()
                        }
                    } else {
                        return
                    }
                }
            }
        }
    });
    sub_manager_sender
}

/// Communicate with a client over TCP
struct TcpAdapter {
    receiver: mpsc::Receiver<Command>,
    sender: mpsc::Sender<Message>,
}
impl TcpAdapter {
    /// Create a new adapter from a tcp stream
    pub fn new(stream: tokio::net::TcpStream) -> TcpAdapter {
        // Split the stream into read and write halfs
        let (stream_reader, stream_writer) = stream.into_split();

        let receiver = TcpAdapter::spawn_read_loop(stream_reader);
        let sender = TcpAdapter::spawn_write_loop(stream_writer);

        TcpAdapter { receiver, sender }
    }

    /// Creates a channel for commands received from the client.
    /// Spawns a task that listens to the stream and forwards any commands received over the channel.
    fn spawn_read_loop(stream_reader: OwnedReadHalf) -> mpsc::Receiver<Command> {
        // A channel for receiving messages from the client.
        let (in_tx, in_rx) = mpsc::channel(config::CHANNEL_BUFFER_SIZE);
        tokio::spawn(async move {
            let codec = MsgPackCodec::<Command>::new();
            let mut framed_reader = FramedRead::new(stream_reader, codec);
            while let Some(result) = framed_reader.next().await {
                match result {
                    Ok(command) => {
                        in_tx
                            .send(command)
                            .await
                            .inspect_err(|e| debug!("Channel send failure - {e}"))
                            .ok();
                    }
                    Err(e) => {
                        debug!("Client disconnected unexpectedly - {:?}", e);
                    }
                }
            }
        });
        in_rx
    }

    /// Creates a channel for messages sent to the client.
    /// Spawns a task that listens to the channela nd forwards any message received over the stream.
    fn spawn_write_loop(stream_writer: OwnedWriteHalf) -> mpsc::Sender<Message> {
        // A channel for sending messages to the client
        let (out_tx, mut out_rx) = mpsc::channel::<Message>(config::CHANNEL_BUFFER_SIZE);
        tokio::spawn(async move {
            let codec = MsgPackCodec::<Message>::new();
            let mut framed_writer = FramedWrite::new(stream_writer, codec);
            while let Some(m) = out_rx.recv().await {
                framed_writer
                    .send(m)
                    .await
                    .inspect_err(|e| debug!("Failed to send to client - {e}"))
                    .ok();
            }
        });
        out_tx
    }
}
impl Adapter for TcpAdapter {
    /// Consume this adapter and convert it into independent sender and receiver channels
    fn into_channels(self) -> (mpsc::Sender<Message>, mpsc::Receiver<Command>) {
        (self.sender, self.receiver)
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
    /// Message bus
    bus: Bus,
}
impl Server {
    /// Create a new server
    pub async fn new(port: u16) -> Result<Server, Box<dyn Error>> {
        let bus = Bus::new();
        Ok(Server { port, bus })
    }

    fn register_metrics(&self) -> MetricsMutator {
        // Command throughput
        let mut command_throughput = metrics::Throughput::new(
            "commands",
            tokio::time::Duration::from_secs(config::M_COMMANDS_INTERVAL_S),
        );
        let command_throughput_mutator = command_throughput.get_mutator();
        let bus = self.bus.clone();
        tokio::spawn(async move {
            command_throughput.listen_and_broadcast(bus.sender).await;
        });

        // Connections metric
        let mut connection_count = metrics::Count::new(
            "connections",
            tokio::time::Duration::from_secs(config::M_CONNECTIONS_INTERVAL_S),
        );
        let connection_count_mutator = connection_count.get_mutator();
        let bus = self.bus.clone();
        tokio::spawn(async move {
            connection_count.listen_and_broadcast(bus.sender).await;
        });

        // Messages sent metric
        let mut messages_sent = metrics::Throughput::new(
            "messages-sent",
            tokio::time::Duration::from_secs(config::M_MESSAGES_SENT_INTERVAL_S),
        );
        let bus = self.bus.clone();
        let messages_sent_mutator = messages_sent.get_mutator();
        tokio::spawn(async move {
            messages_sent.listen_and_broadcast(bus.sender).await;
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

        let metrics = self.register_metrics();

        loop {
            // Accept a new connection
            let (stream, _) = listener.accept().await?;
            let adapter = TcpAdapter::new(stream);
            spawn_connection(adapter, self.bus.clone(), metrics.clone()).await;
        }
    }
}
