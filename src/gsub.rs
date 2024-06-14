use rmp_serde;
use rmp_serde::Serializer;
use rmpv::Value;
use std::error::Error;
use std::hash::Hash;
use std::time::{Duration, Instant};
use tokio;
use tokio::io::{AsyncReadExt,AsyncWriteExt};
use serde::Serialize;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::fs::File;
use futures::future::{join_all, join};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};


use crate::datagram::Message;
use crate::subscription_tree::{self, SubscriptionTree};
use bytes::{Buf, BytesMut};

pub struct MessageReader {
    reader: OwnedReadHalf,
    buffer: BytesMut,
    subscribers: Arc<Mutex<SubscriptionTree<String>>>
}

const SYSTEM_TOPIC_PREFIX: &'static str = "!system";
const SUBSCRIBE_TOPIC: &'static str = "/subscribe";
// const SUBSCRIBE_TOPIC: String = String::from("!system/subscribe");

impl MessageReader {
    pub fn new(stream: OwnedReadHalf, subscribers: Arc<Mutex<SubscriptionTree<String>>>) -> MessageReader {
        MessageReader {
            reader: stream,
            buffer: BytesMut::with_capacity(4096),
            subscribers: subscribers
        }
    }

    async fn receive_loop(&mut self) -> Result<(), Box<dyn Error>> {
        let start = Instant::now();

        let mut count = 0;

        loop {
            let message = self.read_value().await;
            if message.is_err() || message.as_ref().unwrap().is_none() {
                let end = Instant::now();
                let seconds = (end - start).as_millis() as f64 / 1000.0;
                println!(
                    "{} messages received in {}s - {} m/s",
                    count,
                    seconds,
                    (count as f64 / seconds).round()
                );
                println!("Disconnected");
                return Ok(());
            } else {
                // Do something with the message
                // println!("{:?}", message?.unwrap());
                let m : Message = message?.expect("Already checked this is not none");
                // println!("{:?}", m);
                if m.topic().starts_with(SYSTEM_TOPIC_PREFIX) {
                    // TODO: spawn new task
                    // drop the prefix
                    match m.topic().trim_start_matches(SYSTEM_TOPIC_PREFIX) {
                        SUBSCRIBE_TOPIC => {
                            // Message value contains subscription pattern
                            println!("New subscription request: {:?}", (self.reader.peer_addr().unwrap(), &m.value().as_str().unwrap()));
                            // Wait for ownership of mutex lock
                            let mut subscribers = self.subscribers.lock().unwrap();

                            // Add new subscription entry to the subscriber tree
                            subscribers.subscribe(&m.value().as_str().unwrap(), self.reader.peer_addr().unwrap().to_string());

                            
                        },
                        _ => ()
                    }
                };

                // Route message to message handlers
            }

            count += 1;
        }
    }

    fn parse_value(&mut self) -> (Option<Message>, usize) {
        let buf = &mut &self.buffer[..];
        let start_len = buf.len();
        // let v= decode::value::read_value(buf).ok();
        let message = rmp_serde::decode::from_read::<&mut &[u8], Message>(buf).ok();
        let end_len = buf.len();

        (message, (start_len - end_len))
    }

    pub async fn read_value(&mut self) -> Result<Option<Message>, Box<dyn Error>> {
        loop {
            // Attempt to parse a frame from the buffered data. If
            // enough data has been buffered, the frame is
            // returned.
            let (message, bytes_read) = self.parse_value();
            if let Some(m) = message {
                self.buffer.advance(bytes_read);
                return Ok(Some(m));
            }

            // There is not enough buffered data to read a frame.
            // Attempt to read more data from the socket.
            //
            // On success, the number of bytes is returned. `0`
            // indicates "end of stream".
            self.reader.readable().await?;
            let bytes_read = self.reader.read_buf(&mut self.buffer).await?;
            if bytes_read == 0 {
                // The remote closed the connection. For this to be
                // a clean shutdown, there should be no data in the
                // read buffer. If there is, this means that the
                // peer closed the socket while sending a frame.
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err("connection reset by peer".into());
                }
            }
        }
    }
}

pub struct MessageWriter {
    writer: OwnedWriteHalf,
}

impl MessageWriter {
    pub fn new(stream: OwnedWriteHalf) -> MessageWriter {
        MessageWriter { writer: stream }
    }

    pub async fn send(&mut self, message: Message) -> Result<(), Box<dyn Error>> {
        let mut buf = Vec::new();
        message.serialize(&mut Serializer::new(&mut buf)).unwrap();
        self.writer.write(&mut buf).await?;
        Ok(())
    }

    pub async fn write_loop(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
            println!("publishing...");
            self.send(Message::new("test", Value::Boolean(true))).await?;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}

#[derive(Debug, Clone)]
struct Subscription {
    addr: String,
    pattern: String
}

/// An async message passing application
#[derive(Debug)]
pub struct GSub {
    /// Port on which to listen for new requests
    port: u16,
    /// Structure containing information about clients subscribed to topics on this GSub
    subscribers: Arc<Mutex<subscription_tree::SubscriptionTree<String>>>,

    /// List of this GSub's subscriptions to topics on other GSubs
    subscriptions: Vec<Subscription>
}
impl GSub {
    pub async fn new(port: u16, subscription_file: &str) -> Result<GSub, Box<dyn Error>> {
        Ok(GSub {
            port,
            subscribers: Arc::new(Mutex::new(subscription_tree::SubscriptionTree::new())),
            subscriptions: GSub::parse_subscription_config(subscription_file).await?
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
    async fn parse_subscription_config(subscription_file: &str) -> Result<Vec<Subscription>, Box<dyn Error>> {
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
                subscriptions.push(Subscription{addr, pattern});
            });
        Ok(subscriptions)
    }

    /// Listen for incoming connections
    /// 
    pub async fn server(&self) -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port)).await?;

        loop {
            let (stream, _) = listener.accept().await?;
            println!("Connection made");
            let (r, w) = stream.into_split();
            let mut reader = MessageReader::new(r, self.subscribers.clone());
            tokio::spawn(async move {
                _ = reader.receive_loop().await;
            });

            let mut writer = MessageWriter::new(w);
            tokio::spawn(async move {
                _ = writer
                    .send(Message::new("test", Value::Boolean(true)))
                    .await;
            });
        }
    }
    
    /// Loop forever receiving messages from a specific host, attempting to maintain connection with server
    async fn receive(&self, addr: String, subscriptions: Vec<String>) -> () {
        loop {
            let resp = TcpStream::connect(&addr).await;
            match resp {
                Ok(stream) => {
                    println!("Connected: {}", addr);

                    let (r, w) = stream.into_split();
                    let mut reader = MessageReader::new(r, self.subscribers.clone());
                    let read_future = tokio::spawn(async move {
                        _ = reader.receive_loop().await.unwrap();
                    });

                    let mut writer = MessageWriter::new(w);
                    let subs = subscriptions.clone();
                    for s in subs {
                        // println!("Subscribe: {}", s);
                        let topic = format!("{}{}", SYSTEM_TOPIC_PREFIX, SUBSCRIBE_TOPIC);
                        writer.send(Message::new(&topic, Value::from(s))).await.expect("Subscription was sent");
                    }

                    let write_future = tokio::spawn(async move {
                        _ = writer
                            .write_loop()
                            .await.unwrap();
                    });
                    let (r, w) = tokio::join!(read_future, write_future);
                    if r.is_err() { continue };
                    if w.is_err() { continue };
                }
                Err(e) => {
                    println!("{:?}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue
                }
            }
        }
    }

    /// Manage client connections
    pub async fn client(&self) {
        // Not efficient for large number of subscriptions
        let mut futures = vec![];
        let unique_hosts= HashSet::<String>::from_iter(self.subscriptions.iter().map(|s| s.addr.clone()));

        // unique_hosts.
        unique_hosts.into_iter().for_each(|addr| {
            let subscriptions = Vec::from_iter(
                self.subscriptions.iter()
                .filter(|s| s.addr == *addr)
                .map(|s| s.pattern.clone())
            );
            let future = self.receive(addr.clone(), subscriptions);
            futures.push(future);
        });
        join_all(futures).await;
    }

    /// High level entrypoint
    pub async fn run(&self) {
        // Guarded and reference counted reference to subscriber tree
        // let subscribers = Arc::new(Mutex::new(&self.subscribers));

        // Spawn server task to listen for incoming connections
        let server_future = self.server();

        // Spawn client task to make outgoing connections
        let client_future = self.client();
        let (server_result, _) = join(server_future, client_future).await;
        server_result.unwrap();
    }
}
