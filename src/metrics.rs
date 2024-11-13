use std::error::Error;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

use crate::config;

/// Mutate a counter possibly owned by another thread / task
#[derive(Clone)]
pub struct CounterMutator {
    sender: mpsc::Sender<u64>,
}
impl CounterMutator {
    pub async fn increment(&self) {
        self.sender
            .send(1)
            .await
            .expect("Message is sent over channel");
    }
}

pub struct Counter {
    value: u64,
    start: Option<Instant>,
    sender: mpsc::Sender<u64>,
    receiver: mpsc::Receiver<u64>,
}
impl Counter {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(config::CHANNEL_BUFFER_SIZE);
        Self {
            value: 0,
            start: None,
            sender,
            receiver,
        }
    }

    pub fn get_mutator(&self) -> CounterMutator {
        CounterMutator {
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

    pub fn rate(&self) -> Result<f64, Box<dyn Error>> {
        if let Some(start) = self.start {
            let now = Instant::now();
            Ok(self.value as f64 / ((now - start).as_micros() as f64 / 1_000_000.))
        } else {
            Err("Counter hasn't started yet.".into())
        }
    }

    pub fn reset(&mut self) {
        self.value = 0;
        self.start = Some(Instant::now());
    }
}
