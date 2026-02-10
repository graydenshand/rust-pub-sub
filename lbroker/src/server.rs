//! Lion Broker server.
//!
//! This module contains a async pub sub server that sends and receives message pack encoded messages.
//!
//! It is designed to support multiple transport protocols (eg. Tcp, Udp, Websockets).

use crate::buffer_config::BufferConfig;
use log::{debug, warn};

use std::error::Error;
use std::sync::Arc;

use dashmap::DashMap;
use tokio;
use tokio::sync::broadcast;

use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::config;
use crate::interface::{Command, Message, MsgPackCodec};
use crate::metrics;
use crate::metrics::Metric;
use glob_tree::{self};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{StreamExt, StreamMap};
use tokio_util::codec::FramedRead;

/// Events about topic lifecycle that all Bus clones receive
#[derive(Clone, Debug)]
enum TopicEvent {
    Created { name: String },
}

/// Data stored for each topic
///
/// For each topic, we retain the last message sent over it. This helps to address
/// a race condition when a topic is created, and gives clients the most recent
/// state of any topics they're subscribed to immediately.
#[derive(Debug)]
struct TopicData {
    /// Broadcast sender for this topic
    sender: broadcast::Sender<Arc<Vec<u8>>>,
    /// Most recent message for late subscribers
    last_message: Option<Arc<Vec<u8>>>,
}

impl TopicData {
    fn new() -> Self {
        let (sender, _) = broadcast::channel(config::CHANNEL_BUFFER_SIZE);
        Self {
            sender,
            last_message: None,
        }
    }

    fn set_last_message(&mut self, message: Arc<Vec<u8>>) {
        self.last_message = Some(message);
    }

    fn get_last_message(&self) -> Option<Arc<Vec<u8>>> {
        self.last_message.clone()
    }
}

/// A Bus connects one connection to all other connections on the server.
/// Each bus can send and receive messages to all other buses on topics. Each
/// topic has its own channel on the bus for granular message routing.
#[derive(Clone, Debug)]
struct Bus {
    /// Map of topic names to their data (sender + message buffer)
    topics: Arc<DashMap<Arc<str>, TopicData>>,
    /// Sender for broadcasting topic lifecycle events to all Bus clones
    topic_events_tx: broadcast::Sender<TopicEvent>,
}

impl Bus {
    fn new() -> Self {
        let (topic_events_tx, _) = broadcast::channel(config::CHANNEL_BUFFER_SIZE);
        Self {
            topics: Arc::new(DashMap::new()),
            topic_events_tx,
        }
    }

    /// Subscribe to topic lifecycle events (new topics, deleted topics, etc.)
    /// Each process should call this once and listen for new topics
    fn subscribe_to_topic_events(&self) -> broadcast::Receiver<TopicEvent> {
        self.topic_events_tx.subscribe()
    }

    /// Send a message to a topic, creating it if it doesn't exist
    fn send(&self, topic_name: &str, message: Arc<Vec<u8>>) -> Result<(), &str> {
        // Check if topic exists
        if !self.topics.contains_key(topic_name) {
            self.create_topic(topic_name);
        }

        // Send the actual message and add to buffer
        match self.topics.get_mut(topic_name) {
            None => Err("Topic not found"), // Shouldn't happen after creation above
            Some(mut topic_data) => {
                // Store as last message (for late subscribers)
                topic_data.set_last_message(message.clone());

                // Broadcast send returns an error if there are no receivers
                // This is expected and not a failure - the message was "sent"
                topic_data.sender.send(message).ok();
                Ok(())
            }
        }
    }

    /// Create a topic explicitly (can also be called by send())
    fn create_topic(&self, topic_name: &str) -> bool {
        let topic_arc: Arc<str> = Arc::from(topic_name);

        // Check if topic already exists
        if self.topics.contains_key(&topic_arc) {
            return false;
        }

        // Insert the new topic
        self.topics
            .entry(topic_arc.clone())
            .or_insert_with(TopicData::new);

        // Broadcast topic creation event
        self.topic_events_tx
            .send(TopicEvent::Created {
                name: topic_name.to_string(),
            })
            .ok();

        true
    }

    /// Subscribe to a specific topic's messages
    /// Returns None if the topic doesn't exist, or Some with (receiver, last_message)
    fn subscribe_to_topic(
        &self,
        topic_name: &str,
    ) -> Option<(broadcast::Receiver<Arc<Vec<u8>>>, Option<Arc<Vec<u8>>>)> {
        self.topics.get(topic_name).map(|topic_data| {
            let receiver = topic_data.sender.subscribe();
            let last_msg = topic_data.get_last_message();
            (receiver, last_msg)
        })
    }

    /// List all current topics
    fn list_topics(&self) -> Vec<String> {
        self.topics
            .iter()
            .map(|entry| entry.key().to_string())
            .collect()
    }
}

/// An interface that supports receiving commands and sending messages to a client.
trait Adapter {
    /// Consume this adapter and convert it into independent sender and receiver channels
    /// Returns (sender for outgoing message bytes, receiver for incoming commands)
    fn into_channels(self) -> (mpsc::Sender<Arc<Vec<u8>>>, mpsc::Receiver<Command>);
}

/// Spawn a new connection
///
/// The connection bridges an adapter and the bus. It manages subscriptions to topics
/// based on glob patterns and handles topic discovery.
/// Handles a single client connection's state and message routing
struct ConnectionHandler {
    adapter_sender: mpsc::Sender<Arc<Vec<u8>>>,
    adapter_receiver: mpsc::Receiver<Command>,
    bus: Bus,
    topic_streams: StreamMap<Arc<str>, BroadcastStream<Arc<Vec<u8>>>>,
    topic_events: broadcast::Receiver<TopicEvent>,
    subscriptions: glob_tree::GlobTree,
    metrics: MetricsMutator,
}

impl ConnectionHandler {
    fn new<T: Adapter>(adapter: T, bus: Bus, metrics: MetricsMutator) -> Self {
        let (adapter_sender, adapter_receiver) = adapter.into_channels();
        let topic_events = bus.subscribe_to_topic_events();

        Self {
            adapter_sender,
            adapter_receiver,
            bus,
            topic_streams: StreamMap::new(),
            topic_events,
            subscriptions: glob_tree::GlobTree::new(),
            metrics,
        }
    }

    async fn run(mut self) {
        self.metrics.connection_count.add(1).await;

        loop {
            tokio::select! {
                Some(command) = self.adapter_receiver.recv() => {
                    self.metrics.command_throughput.increment().await;
                    debug!("{}", command);

                    if !self.handle_command(command).await {
                        break;
                    }
                }

                Ok(event) = self.topic_events.recv() => {
                    self.handle_topic_event(event);
                }

                Some((topic_name, msg_result)) = self.topic_streams.next() => {
                    match msg_result {
                        Ok(msg_bytes) => {
                            if !self.forward_message_to_client(msg_bytes).await {
                                break;
                            }
                        }
                        Err(e) => {
                            // Broadcast lag error - receiver fell behind
                            warn!("Broadcast lag on topic {}: {}", topic_name, e);
                        }
                    }
                }

                else => break,
            }
        }

        self.metrics.connection_count.subtract(1).await;
    }

    /// Returns false if connection should close
    async fn handle_command(&mut self, command: Command) -> bool {
        match command {
            Command::Subscribe { pattern } => {
                self.handle_subscribe(pattern);
                true
            }
            Command::Unsubscribe { pattern } => {
                self.handle_unsubscribe(pattern);
                true
            }
            Command::Publish { message } => {
                self.handle_publish(message);
                true
            }
        }
    }

    fn handle_subscribe(&mut self, pattern: String) {
        self.subscriptions.insert(&pattern);

        // Subscribe to all existing topics that match this pattern
        for topic_name in self.bus.list_topics() {
            if self.subscriptions.check(&topic_name).is_some() {
                self.subscribe_to_topic(topic_name);
            }
        }
    }

    fn handle_unsubscribe(&mut self, pattern: String) {
        self.subscriptions.remove(&pattern).ok();

        // Unsubscribe from topics that no longer match any pattern
        let subscribed_topics: Vec<Arc<str>> = self.topic_streams.keys().cloned().collect();
        for topic in subscribed_topics {
            if self.subscriptions.check(topic.as_ref()).is_none() {
                self.topic_streams.remove(&topic);
            }
        }
    }

    fn handle_publish(&self, message: Message) {
        // Serialize the message to msgpack bytes
        let mut bytes = Vec::new();
        if let Err(e) = rmp_serde::encode::write(&mut bytes, &message) {
            warn!("Failed to serialize message: {}", e);
            return;
        }

        self.bus
            .send(&message.topic, Arc::new(bytes))
            .inspect_err(|e| warn!("Failed to send message to bus - {e}"))
            .ok();
    }

    fn handle_topic_event(&mut self, event: TopicEvent) {
        match event {
            TopicEvent::Created { name } => {
                self.handle_new_topic(name);
            }
        }
    }

    fn handle_new_topic(&mut self, name: String) {
        // Check if any of our subscription patterns match this new topic
        match self.subscriptions.check(&name) {
            Some(pattern) => {
                // Suppress system topics unless explicitly subscribed
                let is_system_topic = name.starts_with(config::SYSTEM_TOPIC_PREFIX);
                let pattern_matches_system = pattern.starts_with(config::SYSTEM_TOPIC_PREFIX);

                if !is_system_topic || pattern_matches_system {
                    self.subscribe_to_topic(name);
                }
            }
            None => {}
        }
    }

    fn subscribe_to_topic(&mut self, topic_name: String) {
        let topic_arc: Arc<str> = Arc::from(topic_name.as_str());

        // Don't subscribe twice
        if self.topic_streams.contains_key(&topic_arc) {
            return;
        }

        if let Some((receiver, last_msg)) = self.bus.subscribe_to_topic(&topic_name) {
            // Send last message immediately if it exists
            if let Some(msg) = last_msg {
                // We can't await here, so we'll send it on the next stream poll
                // For now, wrap the receiver in BroadcastStream
                let stream = BroadcastStream::new(receiver);
                self.topic_streams.insert(topic_arc.clone(), stream);

                // Immediately forward the last message (synchronously via adapter_sender.try_send)
                // Note: This is a slight behavior change - we send it immediately instead of async
                self.adapter_sender.try_send(msg).ok();
            } else {
                let stream = BroadcastStream::new(receiver);
                self.topic_streams.insert(topic_arc, stream);
            }
        }
    }

    /// Returns false if client disconnected
    async fn forward_message_to_client(&self, msg_bytes: Arc<Vec<u8>>) -> bool {
        self.metrics.messages_sent.increment().await;

        // Send bytes directly to client (no need to deserialize)
        self.adapter_sender.send(msg_bytes).await.is_ok()
    }
}

async fn spawn_connection<T: Adapter + Send + 'static>(
    adapter: T,
    bus: Bus,
    metrics: MetricsMutator,
) {
    tokio::spawn(async move {
        let handler = ConnectionHandler::new(adapter, bus, metrics);
        handler.run().await;
    });
}

/// Communicate with a client over TCP
struct TcpAdapter {
    receiver: mpsc::Receiver<Command>,
    sender: mpsc::Sender<Arc<Vec<u8>>>,
}
impl TcpAdapter {
    /// Create a new adapter from a tcp stream
    pub fn new(stream: tokio::net::TcpStream, buffer_config: BufferConfig) -> TcpAdapter {
        // Split the stream into read and write halfs
        let (stream_reader, stream_writer) = stream.into_split();

        let receiver = TcpAdapter::spawn_read_loop(stream_reader);
        let sender = TcpAdapter::spawn_write_loop(stream_writer, buffer_config);

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

    /// Creates a channel for message bytes sent to the client.
    /// Spawns a task that listens to the channel and forwards any bytes received over the stream.
    fn spawn_write_loop(
        stream_writer: OwnedWriteHalf,
        buffer_config: BufferConfig,
    ) -> mpsc::Sender<Arc<Vec<u8>>> {
        use crate::buffer_config::FlushStrategy;
        use tokio::io::{AsyncWriteExt, BufWriter};

        // A channel for sending message bytes to the client
        let (out_tx, mut out_rx) = mpsc::channel::<Arc<Vec<u8>>>(config::CHANNEL_BUFFER_SIZE);

        tokio::spawn(async move {
            let mut buf_writer = BufWriter::with_capacity(buffer_config.size, stream_writer);
            let mut flush_interval = match buffer_config.flush_strategy {
                FlushStrategy::Periodic { interval_ms } => Some(tokio::time::interval(
                    tokio::time::Duration::from_millis(interval_ms),
                )),
                _ => None,
            };

            loop {
                tokio::select! {
                    Some(bytes) = out_rx.recv() => {
                        if buf_writer.write_all(&bytes).await.is_err() {
                            debug!("Failed to send to client");
                            break;
                        }

                        // Flush based on strategy
                        match buffer_config.flush_strategy {
                            FlushStrategy::Immediate => {
                                if buf_writer.flush().await.is_err() {
                                    debug!("Failed to flush to client");
                                    break;
                                }
                            }
                            FlushStrategy::Auto => {
                                // Flush if no more messages are waiting (prevents small messages from getting stuck)
                                if out_rx.is_empty() {
                                    if buf_writer.flush().await.is_err() {
                                        debug!("Failed to flush to client");
                                        break;
                                    }
                                }
                            }
                            FlushStrategy::Periodic { .. } => {
                                // Flush on interval (handled in select branch below)
                            }
                        }
                    }
                    Some(_) = async {
                        match &mut flush_interval {
                            Some(interval) => Some(interval.tick().await),
                            None => None,
                        }
                    } => {
                        if buf_writer.flush().await.is_err() {
                            debug!("Failed to periodic flush to client");
                            break;
                        }
                    }
                    else => break,
                }
            }
        });
        out_tx
    }
}
impl Adapter for TcpAdapter {
    /// Consume this adapter and convert it into independent sender and receiver channels
    fn into_channels(self) -> (mpsc::Sender<Arc<Vec<u8>>>, mpsc::Receiver<Command>) {
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
    /// Buffer configuration for write buffering
    buffer_config: BufferConfig,
}
impl Server {
    /// Create a new server
    pub async fn new(port: u16, buffer_config: BufferConfig) -> Result<Server, Box<dyn Error>> {
        let bus = Bus::new();

        // Create system metrics topic
        bus.create_topic(&format!("{}/metrics", config::SYSTEM_TOPIC_PREFIX));

        Ok(Server {
            port,
            bus,
            buffer_config,
        })
    }

    fn register_metrics(&self) -> MetricsMutator {
        // Create a wrapper that serializes Message to bytes and sends via Bus
        let bus = self.bus.clone();
        let (metrics_tx, mut metrics_rx) = mpsc::channel::<Message>(100);

        tokio::spawn(async move {
            while let Some(message) = metrics_rx.recv().await {
                // Serialize the message to bytes
                match rmp_serde::to_vec(&message) {
                    Ok(bytes) => {
                        bus.send(&message.topic, Arc::new(bytes))
                            .inspect_err(|e| warn!("Failed to send metric - {e}"))
                            .ok();
                    }
                    Err(e) => {
                        warn!("Failed to serialize metric message - {e}");
                    }
                }
            }
        });

        // Create a broadcast channel for the metrics
        let (metrics_sender, _) = broadcast::channel(100);
        let metrics_sender_clone = metrics_sender.clone();

        // Forward from broadcast to mpsc (for serialization)
        tokio::spawn(async move {
            let mut receiver = metrics_sender_clone.subscribe();
            while let Ok(msg) = receiver.recv().await {
                metrics_tx.send(msg).await.ok();
            }
        });

        // Command throughput
        let mut command_throughput = metrics::Throughput::new(
            "commands",
            tokio::time::Duration::from_secs(config::M_COMMANDS_INTERVAL_S),
        );
        let command_throughput_mutator = command_throughput.get_mutator();
        let sender = metrics_sender.clone();
        tokio::spawn(async move {
            command_throughput.listen_and_broadcast(sender).await;
        });

        // Connections metric
        let mut connection_count = metrics::Count::new(
            "connections",
            tokio::time::Duration::from_secs(config::M_CONNECTIONS_INTERVAL_S),
        );
        let connection_count_mutator = connection_count.get_mutator();
        let sender = metrics_sender.clone();
        tokio::spawn(async move {
            connection_count.listen_and_broadcast(sender).await;
        });

        // Messages sent metric
        let mut messages_sent = metrics::Throughput::new(
            "messages-sent",
            tokio::time::Duration::from_secs(config::M_MESSAGES_SENT_INTERVAL_S),
        );
        let sender = metrics_sender.clone();
        let messages_sent_mutator = messages_sent.get_mutator();
        tokio::spawn(async move {
            messages_sent.listen_and_broadcast(sender).await;
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
        let buffer_config = self.buffer_config;

        loop {
            let (socket, _) = listener.accept().await?;
            let bus = self.bus.clone();

            let adapter = TcpAdapter::new(socket, buffer_config);
            spawn_connection(adapter, bus, metrics.clone()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bus_topic_creation_and_subscription() {
        let bus = Bus::new();

        // Create a topic
        let topic_name = "test.topic";
        assert!(bus.create_topic(topic_name));

        // Creating the same topic again should return false
        assert!(!bus.create_topic(topic_name));

        // Should be able to subscribe to it
        let result = bus.subscribe_to_topic(topic_name);
        assert!(result.is_some());
        let (_receiver, last_msg) = result.unwrap();
        assert!(last_msg.is_none()); // No messages yet

        // Should be in the list of topics
        let topics = bus.list_topics();
        assert!(topics.contains(&topic_name.to_string()));
    }

    #[tokio::test]
    async fn test_bus_send_creates_topic() {
        let bus = Bus::new();

        let topic_name = "auto.created.topic";
        let message = Arc::new(vec![1, 2, 3, 4]);

        // Sending to a non-existent topic should create it
        assert!(bus.send(topic_name, message.clone()).is_ok());

        // Topic should now exist
        let topics = bus.list_topics();
        assert!(topics.contains(&topic_name.to_string()));
    }

    #[tokio::test]
    async fn test_bus_topic_events() {
        let bus = Bus::new();
        let mut events = bus.subscribe_to_topic_events();

        // Create a topic
        let topic_name = "event.test";
        bus.create_topic(topic_name);

        // Should receive a Created event
        tokio::select! {
            Ok(event) = events.recv() => {
                match event {
                    TopicEvent::Created { name } => {
                        assert_eq!(name, topic_name);
                    }
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                panic!("Did not receive topic event");
            }
        }
    }

    #[tokio::test]
    async fn test_stream_map_unified_stream() {
        let bus = Bus::new();
        let mut topic_streams = StreamMap::new();

        // Create and subscribe to multiple topics
        let topic1 = "topic.one";
        let topic2 = "topic.two";

        bus.create_topic(topic1);
        bus.create_topic(topic2);

        let (receiver1, _last_msg1) = bus.subscribe_to_topic(topic1).unwrap();
        let (receiver2, _last_msg2) = bus.subscribe_to_topic(topic2).unwrap();

        topic_streams.insert(Arc::from(topic1), BroadcastStream::new(receiver1));
        topic_streams.insert(Arc::from(topic2), BroadcastStream::new(receiver2));

        // Send messages to both topics
        let msg1 = Arc::new(vec![1, 2, 3]);
        let msg2 = Arc::new(vec![4, 5, 6]);

        bus.send(topic1, msg1.clone()).ok();
        bus.send(topic2, msg2.clone()).ok();

        // Should receive both messages from the unified stream
        let mut received = vec![];
        for _ in 0..2 {
            tokio::select! {
                Some((topic, _msg_result)) = topic_streams.next() => {
                    received.push(topic);
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    panic!("Did not receive message");
                }
            }
        }

        assert!(received.iter().any(|t: &Arc<str>| t.as_ref() == topic1));
        assert!(received.iter().any(|t: &Arc<str>| t.as_ref() == topic2));
    }

    #[tokio::test]
    async fn test_stream_map_unsubscribe() {
        let bus = Bus::new();
        let mut topic_streams = StreamMap::new();

        let topic = "unsubscribe.test";
        bus.create_topic(topic);

        let (receiver, _last_msg) = bus.subscribe_to_topic(topic).unwrap();
        topic_streams.insert(Arc::from(topic), BroadcastStream::new(receiver));

        // Verify subscribed
        assert_eq!(topic_streams.len(), 1);

        // Unsubscribe
        topic_streams.remove(topic);

        // Verify unsubscribed
        assert_eq!(topic_streams.len(), 0);
    }

    #[tokio::test]
    async fn test_message_buffering_for_late_subscribers() {
        let bus = Bus::new();
        let topic = "buffered.topic";

        // Create topic and send messages BEFORE anyone subscribes
        bus.create_topic(topic);
        let msg1 = Arc::new(vec![1, 2, 3]);
        let msg2 = Arc::new(vec![4, 5, 6]);
        let msg3 = Arc::new(vec![7, 8, 9]);

        bus.send(topic, msg1.clone()).ok();
        bus.send(topic, msg2.clone()).ok();
        bus.send(topic, msg3.clone()).ok();

        // Now subscribe - should receive last message only
        let (mut receiver, last_msg) = bus.subscribe_to_topic(topic).unwrap();

        // Check last message (only the most recent one is buffered)
        assert!(last_msg.is_some());
        assert_eq!(*last_msg.unwrap(), vec![7, 8, 9]);

        // Send a new message after subscription
        let msg4 = Arc::new(vec![10, 11, 12]);
        bus.send(topic, msg4.clone()).ok();

        // Should receive it live
        tokio::select! {
            Ok(msg) = receiver.recv() => {
                assert_eq!(*msg, vec![10, 11, 12]);
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                panic!("Did not receive live message");
            }
        }
    }

    #[tokio::test]
    async fn test_race_condition_prevention_with_buffering() {
        // This test simulates the exact scenario you described:
        // 1. Client connects and subscribes to pattern "sensor.*"
        // 2. Another client publishes to "sensor.temperature" (new topic)
        // 3. First client should still receive the message via buffering

        let bus = Bus::new();
        let mut topic_streams: StreamMap<Arc<str>, BroadcastStream<Arc<Vec<u8>>>> =
            StreamMap::new();
        let mut topic_events = bus.subscribe_to_topic_events();

        // Client subscribes to pattern "sensor.*" before any sensor topics exist
        let mut subscriptions = glob_tree::GlobTree::new();
        subscriptions.insert("sensor.*");

        // No sensor topics exist yet, so nothing to subscribe to
        assert_eq!(bus.list_topics().len(), 0);

        // Another client publishes to "sensor.temperature" (creating the topic)
        let message = Arc::new(vec![42, 43, 44]);
        bus.send("sensor.temperature", message.clone()).ok();

        // The first client receives the topic creation event
        let topic_name = tokio::select! {
            Ok(TopicEvent::Created { name }) = topic_events.recv() => name,
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                panic!("Did not receive topic creation event");
            }
        };

        // Check if the new topic matches our subscription pattern
        assert_eq!(topic_name, "sensor.temperature");
        assert!(subscriptions.check(&topic_name).is_some());

        // Subscribe to the topic (this happens AFTER the message was sent)
        // The last_msg should contain the buffered message
        if let Some((receiver, last_msg)) = bus.subscribe_to_topic(&topic_name) {
            // Verify we got the buffered message
            assert!(last_msg.is_some());
            assert_eq!(*last_msg.unwrap(), vec![42, 43, 44]);

            topic_streams.insert(
                Arc::from(topic_name.as_str()),
                BroadcastStream::new(receiver),
            );
        }
    }
}
