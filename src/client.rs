use tokio::net::tcp::OwnedReadHalf;
use tokio::net::TcpStream;
// use uuid::Uuid;

use crate::datagram::{
    Message, MessageReader, MessageWriter, SUBSCRIBE_TOPIC, SYSTEM_TOPIC_PREFIX,
};
use rmpv::{Value, Utf8String};

use futures::future::join_all;
// use rmpv::Value;
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

// Delay before retrying server connections upon failure, in milliseconds
const SERVER_POLL_INTERVAL_MS: u64 = 1000;

/// An async message passing application
#[derive(Debug)]
pub struct Client {
    /// Unique ID for this client, used to register subscriptions & route messages on server
    // id: String,
    /// List of this Client's subscriptions
    subscriptions: Vec<Subscription>,
}
impl Client {
    pub fn new() -> Client {
        Client {
            subscriptions: Vec::new(),
            // id: Uuid::new_v4().to_string(),
        }
    }

    pub async fn subscribe(&mut self, addr: &str, pattern: &str) -> Result<(), Box<dyn Error>> {
        self.subscriptions.push(Subscription{ addr: addr.to_string(), pattern: pattern.to_string() });
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

    async fn receive_loop(stream: OwnedReadHalf) -> Result<(), Box<dyn Error>> {
        let mut reader = MessageReader::new(stream);
        loop {
            let message = reader.read_value().await;
            if message.is_err() || message.as_ref().unwrap().is_none() {
                println!("Disconnected");
                return Ok(());
            } else {
                let m: Message = message.expect("message is Ok").expect("message is not None");
                println!("{m:?}");
            }
        }
    }

    /// Loop forever receiving messages from a specific host, attempting to maintain connection with server
    async fn server_connection(&self, addr: String, subscriptions: Vec<String>) -> () {
        loop {
            // let resp = TcpStream::connect(&addr).await;
            // match resp {
            //     Ok(stream) => {
            //         println!("Connected: {}", addr);

            //         let (r, w) = stream.into_split();
                    
            //         let read_future = tokio::spawn(async move {
            //             _ = Client::receive_loop(r).await.unwrap();
            //         });

            //         let mut writer = MessageWriter::new(w);
            //         let subs = subscriptions.clone();
            //         for s in subs {
            //             // println!("Subscribe: {}", s);
            //             let topic = format!("{}{}", SYSTEM_TOPIC_PREFIX, SUBSCRIBE_TOPIC);
            //             writer
            //                 .send(Message::new(&topic, Value::from(s)))
            //                 .await
            //                 .expect("Subscription was sent");
            //         }

            //         let write_future = tokio::spawn(async move {
            //             _ = writer.write_loop().await;
            //         });
            //         let (r, w) = tokio::join!(read_future, write_future);
            //         if r.is_err() {
            //             continue;
            //         };
            //         if w.is_err() {
            //             continue;
            //         };
            //     }
            //     Err(e) => {
            //         println!("{:?}", e);
            //         tokio::time::sleep(Duration::from_millis(SERVER_POLL_INTERVAL_MS)).await;
            //         continue;
            //     }
            // }
        }
    }

    /// Publish a stream of messages to a broker
    pub async fn publish<T: Iterator<Item=Message>>(&self, addr: &str, stream: T) {
        let conn = TcpStream::connect(&addr).await.expect("Unable to connect to server");

        let (_, w) = conn.into_split();

        let mut writer = MessageWriter::new(w);

        // writer.send(Message::new("!system/client-id-registration", Value::String(Utf8String::from(self.id.clone()))));

        for message in stream {
            println!("Sending message {message:?}");
            writer.send(message).await.unwrap();
        }
    }

    /// Manage client connections
    pub async fn run(&self) {
        let mut futures = vec![];
        let unique_hosts =
            HashSet::<String>::from_iter(self.subscriptions.iter().map(|s| s.addr.clone()));

        unique_hosts.into_iter().for_each(|addr| {
            let subscriptions = Vec::from_iter(
                self.subscriptions
                    .iter()
                    .filter(|s| s.addr == *addr)
                    .map(|s| s.pattern.clone()),
            );
            let future = self.server_connection(addr.clone(), subscriptions);
            futures.push(future);
        });
        join_all(futures).await;
    }

}
