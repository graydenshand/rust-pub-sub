use std::error::Error;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

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
    value: u64,
    start: Option<Instant>,
    sender: mpsc::Sender<u64>,
    receiver: mpsc::Receiver<u64>,
}
impl Throughput {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(config::CHANNEL_BUFFER_SIZE);
        Self {
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
    value: u64,
    sender: mpsc::Sender<i64>,
    receiver: mpsc::Receiver<i64>,
}
impl Count {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(config::CHANNEL_BUFFER_SIZE);
        Self {
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
}
