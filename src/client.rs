
use tokio::net::TcpStream;


use crate::datagram::{Message, MessageReader, MessageWriter, SYSTEM_TOPIC_PREFIX, SUBSCRIBE_TOPIC};

use futures::future::join_all;
use rmpv::Value;
use std::collections::HashSet;
use std::error::Error;

use std::time::Duration;
use tokio;
use tokio::fs::File;
use tokio::io::AsyncReadExt;


#[derive(Debug, Clone)]
struct Subscription {
    addr: String,
    pattern: String,
}

/// An async message passing application
#[derive(Debug)]
pub struct Client {
    /// List of this Client's subscriptions
    subscriptions: Vec<Subscription>,
}
impl Client {
    pub async fn new(subscription_file: &str) -> Result<Client, Box<dyn Error>> {
        Ok(Client {
            subscriptions: Client::parse_subscription_config(subscription_file).await?,
        })
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
    async fn parse_subscription_config(
        subscription_file: &str,
    ) -> Result<Vec<Subscription>, Box<dyn Error>> {
        let mut file = File::open(subscription_file).await?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).await?;
        let mut subscriptions = vec![];
        contents
            .lines()
            .filter(|l| !l.trim_start().starts_with("#") && *l != "")
            .for_each(|l| {
                let mut splits = l.split_whitespace();
                let addr = String::from(splits.next().unwrap());
                let pattern = splits.collect::<String>();
                subscriptions.push(Subscription { addr, pattern });
            });
        Ok(subscriptions)
    }

    /// Loop forever receiving messages from a specific host, attempting to maintain connection with server
    async fn receive(&self, addr: String, subscriptions: Vec<String>) -> () {
        loop {
            let resp = TcpStream::connect(&addr).await;
            match resp {
                Ok(stream) => {
                    println!("Connected: {}", addr);

                    let (r, w) = stream.into_split();
                    let mut reader = MessageReader::new(r);
                    let read_future = tokio::spawn(async move {
                        _ = reader.receive_loop().await.unwrap();
                    });

                    let mut writer = MessageWriter::new(w);
                    let subs = subscriptions.clone();
                    for s in subs {
                        // println!("Subscribe: {}", s);
                        let topic = format!("{}{}", SYSTEM_TOPIC_PREFIX, SUBSCRIBE_TOPIC);
                        writer
                            .send(Message::new(&topic, Value::from(s)))
                            .await
                            .expect("Subscription was sent");
                    }

                    let write_future = tokio::spawn(async move {
                        _ = writer.write_loop().await;
                    });
                    let (r, w) = tokio::join!(read_future, write_future);
                    if r.is_err() {
                        continue;
                    };
                    if w.is_err() {
                        continue;
                    };
                }
                Err(e) => {
                    println!("{:?}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            }
        }
    }

    /// Manage client connections
    pub async fn client(&self) {
        // Not efficient for large number of subscriptions
        let mut futures = vec![];
        let unique_hosts =
            HashSet::<String>::from_iter(self.subscriptions.iter().map(|s| s.addr.clone()));

        // unique_hosts.
        unique_hosts.into_iter().for_each(|addr| {
            let subscriptions = Vec::from_iter(
                self.subscriptions
                    .iter()
                    .filter(|s| s.addr == *addr)
                    .map(|s| s.pattern.clone()),
            );
            let future = self.receive(addr.clone(), subscriptions);
            futures.push(future);
        });
        join_all(futures).await;
    }

    /// High level entrypoint
    pub async fn run(&self) {
        let _ = self.client().await;
    }
}
