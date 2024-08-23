use tokio::net::tcp::OwnedReadHalf;
use tokio::net::TcpStream;
// use uuid::Uuid;

use crate::datagram::{
    Message, MessageReader, MessageWriter, SUBSCRIBE_TOPIC, SYSTEM_TOPIC_PREFIX,
};
use rmpv::{Value};
use std::collections::HashMap;

use tokio::sync::mpsc;


// use rmpv::Value;

use std::error::Error;
use tokio::sync::mpsc::{Receiver, Sender};


use tokio;



#[derive(Debug, Clone)]
pub struct Subscription {
    addr: String,
    pattern: String,
}

pub struct Connection {
    tx: Sender<Message>,
    rx: Option<Receiver<Message>>,
    addr: String,
}
impl Clone for Connection {
    fn clone(&self) -> Self {
        Connection {
            tx: self.tx.clone(),
            rx: None, // only one owner of rx allowed
            addr: self.addr.clone(),
        }
    }
}
impl Connection {
    pub async fn publish(&self, message: Message) {
        self.tx.send(message).await.expect("Message was sent")
    }

    pub async fn recv(&mut self) -> Option<Message> {
        if self.rx.is_some() {
            self.rx.as_mut().unwrap().recv().await
        } else {
            panic!("Cannot call recv on a clone")
        }
    }
}

// Delay before retrying server connections upon failure, in milliseconds
const SERVER_POLL_INTERVAL_MS: u64 = 1000;

/// An async message passing application
#[derive(Debug, Clone)]
pub struct Client {
    /// List of this Client's subscriptions
    subscriptions: Vec<Subscription>,
    /// Map of addresses to write channels
    write_channel_map: HashMap<String, Sender<Message>>,
}
impl Client {
    pub fn new() -> Client {
        Client {
            subscriptions: Vec::new(),
            // id: Uuid::new_v4().to_string(),
            write_channel_map: HashMap::new(),
        }
    }

    pub async fn subscribe(&mut self, addr: &str, pattern: &str) -> Result<(), Box<dyn Error>> {
        self.subscriptions.push(Subscription {
            addr: addr.to_string(),
            pattern: pattern.to_string(),
        });
        Ok(())
    }

    /// Parse subscriptions.cfg file containing list of incoming subscriptions
    ///
    /// Args:
    ///     subscription_file: path to subscription.cfg file
    ///
    /// Example subscription.cfg
    /// ```
    /// # Subscribe to all topics on local server
    /// localhost:1996 *
    /// # Subscribe to a specific topic on a remote server
    /// example.com:2468 US/VT/Weather/Daily
    /// ```
    // async fn parse_subscription_config(
    //     subscription_file: &str,
    // ) -> Result<Vec<Subscription>, Box<dyn Error>> {
    //     let mut file = File::open(subscription_file).await?;
    //     let mut contents = String::new();
    //     file.read_to_string(&mut contents).await?;
    //     let mut subscriptions = vec![];
    //     contents
    //         .lines()
    //         .filter(|l| !l.trim_start().starts_with("#") && *l != "")
    //         .for_each(|l| {
    //             let mut splits = l.split_whitespace();
    //             let addr = String::from(splits.next().unwrap());
    //             let pattern = splits.collect::<String>();
    //             subscriptions.push(Subscription { addr, pattern });
    //         });
    //     Ok(subscriptions)
    // }

    async fn receive_loop(
        stream: OwnedReadHalf,
        tx: Sender<Message>,
    ) -> Result<(), Box<dyn Error>> {
        let mut reader = MessageReader::new(stream);
        loop {
            let message = reader.read_value().await?;
            if let Some(m) = message {
                tx.send(m).await?;
            } else {
                return Ok(());
            }
        }
    }

    /// Loop forever receiving messages from a specific host, attempting to maintain connection with server
    pub async fn connect(&mut self, addr: String, subscriptions: Vec<String>) -> Connection {
        let (tx, rx) = mpsc::channel(32);
        let resp = TcpStream::connect(&addr).await;
        match resp {
            Ok(stream) => {
                println!("Connected: {}", &addr);
                let (r, w) = stream.into_split();

                tokio::spawn(async move {
                    _ = Client::receive_loop(r, tx).await.unwrap();
                });

                // write channel
                let (wtx, wrx) = mpsc::channel(32);
                // self.write_channel_map.insert(addr.clone(), wtx);
                let mut writer = MessageWriter::new(w);
                tokio::spawn(async move {
                    writer.write_loop(wrx).await;
                });
                // let (r,w) = tokio::join!(read_future, write_future);
                // r.unwrap();
                // w.unwrap();
                let subs = subscriptions.clone();
                for s in subs {
                    // println!("Subscribe: {}", s);
                    let topic = format!("{}{}", SYSTEM_TOPIC_PREFIX, SUBSCRIBE_TOPIC);
                    wtx.send(Message::new(&topic, Value::from(s)))
                        .await
                        .expect("Message was sent");
                }
                Connection {
                    tx: wtx,
                    rx: Some(rx),
                    addr,
                }
            }
            Err(e) => {
                panic!("{e}");
            }
        }
    }

    // pub async fn publish(&self, addr: &str, message: Message) {
    //     if let Some(tx) = self.write_channel_map.get(addr) {
    //         tx.send(message).await.unwrap();
    //     }
    // }

    /// Publish a stream of messages to a broker
    pub async fn publish_stream<T: Iterator<Item = Message>>(&self, addr: &str, stream: T) {
        let conn = TcpStream::connect(&addr)
            .await
            .expect("Unable to connect to server");

        let (_, w) = conn.into_split();

        let mut writer = MessageWriter::new(w);

        // writer.send(Message::new("!system/client-id-registration", Value::String(Utf8String::from(self.id.clone()))));

        for message in stream {
            println!("Sending message {message:?}");
            writer.send(message).await.unwrap();
        }
    }

    // Manage client connections
    // pub async fn run<F>(&mut self, f: F) where F: Fn(Message) {
    //     let mut futures = vec![];
    //     let unique_hosts =
    //         HashSet::<String>::from_iter(self.subscriptions.iter().map(|s| s.addr.clone()));
    //     let (tx, mut rx) = mpsc::channel(32);

    //     for addr in unique_hosts.into_iter() {
    //         let subscriptions = Vec::from_iter(
    //             self.subscriptions
    //                 .iter()
    //                 .filter(|s| s.addr == *addr)
    //                 .map(|s| s.pattern.clone()),
    //         );
    //         let future = self.connect(addr, subscriptions, tx.clone());
    //         futures.push(future);
    //     };

    //     // while let Some(message) = rx.recv().await {
    //     //     f(message);
    //     // }
    //     join_all(futures).await;
    // }
}
