//! Safe cross task metrics

use crate::interface::Message;
use log::warn;
use rmpv::Value;
use std::error::Error;
use tokio::sync::{broadcast, mpsc};
use tokio::time::Instant;

use crate::config;

pub trait Metric {
    type Mutator;
    type Value;

    /// Create a new instance of the metric
    fn new(name: &str, interval: tokio::time::Duration) -> Self;

    /// Listen for updates from associated mutators
    async fn listen(&mut self) -> Result<(), Box<dyn Error>>;

    /// Broadcast metric to channel
    fn broadcast(&mut self, message_sender: &broadcast::Sender<Message>);

    /// Interval between broadcasts of this metric
    fn broadcast_interval(&self) -> tokio::time::Duration;

    /// Listen for updates and broadcast to channel on a set interval
    async fn listen_and_broadcast(&mut self, message_sender: broadcast::Sender<Message>) {
        let interval = self.broadcast_interval();
        loop {
            tokio::select! {
                // Listen for mutations from connections
                result = self.listen() => {
                    result.unwrap()
                }
                // Wake up periodically to log metrics
                _ = tokio::time::sleep(interval) => {
                    self.broadcast(&message_sender)
                }
            }
        }
    }

    /// Get the value of this metric
    fn value(&self) -> &Self::Value;

    /// Get a mutator for this metric
    fn get_mutator(&self) -> Self::Mutator;
}

/// Mutate a Throughput metric
#[derive(Clone)]
pub struct ThroughputMutator {
    sender: mpsc::Sender<u64>,
}
impl ThroughputMutator {
    pub async fn increment(&self) {
        self.sender
            .send(1)
            .await
            .expect("Message is sent over channel");
    }
}

/// Throughput metric tracks both the total volume and the rate per second
///
/// Example, how many requests did a website receive in a 30s window?
///
/// Use the get_mutator() method to get an object for mutating this metric from
/// another task.
pub struct Throughput {
    name: String,
    value: u64,
    start: Option<Instant>,
    sender: mpsc::Sender<u64>,
    receiver: mpsc::Receiver<u64>,
    interval: tokio::time::Duration,
}
impl Metric for Throughput {
    type Mutator = ThroughputMutator;
    type Value = u64;

    /// Create a new Throughput metric
    fn new(name: &str, interval: tokio::time::Duration) -> Self {
        let (sender, receiver) = mpsc::channel(config::CHANNEL_BUFFER_SIZE);
        Self {
            name: name.to_string(),
            value: 0,
            start: None,
            sender,
            receiver,
            interval,
        }
    }

    /// Get the value of this metric
    fn value(&self) -> &Self::Value {
        &self.value
    }

    /// Get a ThroughputMutator for this instance
    fn get_mutator(&self) -> Self::Mutator {
        ThroughputMutator {
            sender: self.sender.clone(),
        }
    }

    /// Listen for updates
    async fn listen(&mut self) -> Result<(), Box<dyn Error>> {
        self.start = Some(Instant::now());
        loop {
            let i = self.receiver.recv().await.unwrap();
            self.increment(i);
        }
    }

    /// Broadcast metric to channel
    fn broadcast(&mut self, message_sender: &broadcast::Sender<Message>) {
        // Report value and rate
        broadcast_metric(&message_sender, &self.name, self.value().clone());
        broadcast_metric(
            &message_sender,
            &format!("{}-per-second", self.name),
            self.rate(),
        );
        // Reset the metric
        self.reset();
    }

    /// Interval to wait between broadcasts
    fn broadcast_interval(&self) -> tokio::time::Duration {
        self.interval
    }
}
impl Throughput {
    pub fn increment(&mut self, by: u64) {
        self.value += by;
    }

    pub fn rate(&self) -> f64 {
        if let Some(start) = self.start {
            let now = Instant::now();
            self.value as f64 / ((now - start).as_micros() as f64 / 1_000_000.)
        } else {
            0.
        }
    }

    pub fn reset(&mut self) {
        self.value = 0;
        self.start = Some(Instant::now());
    }
}

/// Mutate a Count metric
#[derive(Clone)]
pub struct CountMutator {
    sender: mpsc::Sender<i64>,
}
impl CountMutator {
    pub async fn add(&self, n: i64) {
        self.sender
            .send(n)
            .await
            .expect("Message is sent over channel");
    }

    pub async fn subtract(&self, n: i64) {
        self.sender
            .send(n * -1)
            .await
            .expect("Message is sent over channel");
    }
}

/// A metric for keeping a count of things, e.g. the number of active connections
pub struct Count {
    name: String,
    value: u64,
    sender: mpsc::Sender<i64>,
    receiver: mpsc::Receiver<i64>,
    interval: tokio::time::Duration,
}
impl Metric for Count {
    type Mutator = CountMutator;
    type Value = u64;

    /// Create a new count metric
    fn new(name: &str, interval: tokio::time::Duration) -> Self {
        let (sender, receiver) = mpsc::channel(config::CHANNEL_BUFFER_SIZE);
        Self {
            name: name.to_string(),
            value: 0,
            sender,
            receiver,
            interval,
        }
    }

    /// Get a mutator for the metric
    fn get_mutator(&self) -> Self::Mutator {
        CountMutator {
            sender: self.sender.clone(),
        }
    }

    /// Listen for updates from mutators
    async fn listen(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
            let i = self.receiver.recv().await.unwrap();
            if i >= 0 {
                self.add(i as u64);
            } else {
                self.subtract((i * -1) as u64);
            }
        }
    }

    /// Get the metric value
    fn value(&self) -> &u64 {
        &self.value
    }

    /// Broadcast this metric to a channel
    fn broadcast(&mut self, message_sender: &broadcast::Sender<Message>) {
        broadcast_metric(&message_sender, &self.name, self.value().clone());
    }

    /// Interval to wait between broadcasts of this metric
    fn broadcast_interval(&self) -> tokio::time::Duration {
        self.interval
    }
}
impl Count {
    /// Add to the count
    pub fn add(&mut self, n: u64) {
        self.value += n;
    }

    /// Subtract from the count
    pub fn subtract(&mut self, n: u64) {
        self.value -= n;
    }
}

/// Broadcast a metric to a channel
fn broadcast_metric<T>(message_sender: &broadcast::Sender<Message>, metric: &str, value: T)
where
    rmpv::Value: From<T>,
{
    let m1 = Message {
        topic: format!("{}/metrics/{metric}", config::SYSTEM_TOPIC_PREFIX),
        value: Value::from(value.into()),
    };
    message_sender
        .send(m1)
        .inspect_err(|e| warn!("Failed to public metric - {metric} - {e}"))
        .ok();
}
