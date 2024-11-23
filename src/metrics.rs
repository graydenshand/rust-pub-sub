use crate::interface::Message;
use log::warn;
use rmpv::Value;
use std::error::Error;
use tokio::sync::{broadcast, mpsc};
use tokio::time::Instant;

use crate::config;

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
}
impl Throughput {
    pub fn new(name: &str) -> Self {
        let (sender, receiver) = mpsc::channel(config::CHANNEL_BUFFER_SIZE);
        Self {
            name: name.to_string(),
            value: 0,
            start: None,
            sender,
            receiver,
        }
    }

    pub fn get_mutator(&self) -> ThroughputMutator {
        ThroughputMutator {
            sender: self.sender.clone(),
        }
    }

    pub async fn listen(&mut self) -> Result<(), Box<dyn Error>> {
        self.start = Some(Instant::now());
        loop {
            let i = self.receiver.recv().await.unwrap();
            self.increment(i);
        }
    }

    pub fn value(&self) -> &u64 {
        &self.value
    }

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

    /// Listen for messages, and also publish metrics to broadcast channel
    pub async fn listen_and_broadcast(&mut self, message_sender: broadcast::Sender<Message>) {
        loop {
            tokio::select! {
                // Listen for mutations from connections
                result = self.listen() => {
                    result.unwrap()
                }
                // Wake up periodically to log metrics
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(config::M_COMMANDS_INTERVAL_S)) => {
                    // Broadcast the count and calculated rate
                    broadcast_metric(&message_sender, &self.name, self.value().clone());
                    broadcast_metric(&message_sender, &format!("{}-per-second", self.name), self.rate());

                    // Reset the metric
                    self.reset();
                }
            }
        }
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
}
impl Count {
    pub fn new(name: &str) -> Self {
        let (sender, receiver) = mpsc::channel(config::CHANNEL_BUFFER_SIZE);
        Self {
            name: name.to_string(),
            value: 0,
            sender,
            receiver,
        }
    }

    pub fn get_mutator(&self) -> CountMutator {
        CountMutator {
            sender: self.sender.clone(),
        }
    }

    pub async fn listen(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
            let i = self.receiver.recv().await.unwrap();
            if i >= 0 {
                self.add(i as u64);
            } else {
                self.subtract((i * -1) as u64);
            }
        }
    }

    pub fn value(&self) -> &u64 {
        &self.value
    }

    pub fn add(&mut self, n: u64) {
        self.value += n;
    }

    pub fn subtract(&mut self, n: u64) {
        self.value -= n;
    }

    pub async fn listen_and_broadcast(&mut self, message_sender: broadcast::Sender<Message>) {
        loop {
            tokio::select! {
                // Listen for mutations from connections
                result = self.listen() => {
                    result.unwrap()
                }
                // Wake up periodically to log metrics
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(config::M_COMMANDS_INTERVAL_S)) => {
                    broadcast_metric(&message_sender, &self.name, self.value().clone());
                }
            }
        }
    }
}

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
