use rmpv::Value;

use std::error::Error;

use std::sync::{Arc, Mutex};

use tokio;

use tokio::net::tcp::OwnedReadHalf;
use tokio::net::TcpListener;

use crate::datagram::{
    Message, MessageReader, MessageWriter, SUBSCRIBE_TOPIC, SYSTEM_TOPIC_PREFIX,
};
use crate::subscription_tree::{self};

#[derive(Debug, Clone)]
struct Subscription {
    addr: String,
    pattern: String,
}

/// An async message passing application
#[derive(Debug)]
pub struct Server {
    /// Port on which to listen for new requests
    port: u16,
    /// Structure containing information about clients subscribed to topics on this Server
    subscribers: Arc<Mutex<subscription_tree::SubscriptionTree<String>>>,
}
impl Server {
    pub async fn new(port: u16) -> Result<Server, Box<dyn Error>> {
        Ok(Server {
            port,
            subscribers: Arc::new(Mutex::new(subscription_tree::SubscriptionTree::new())),
        })
    }

    async fn on_disconnect(&mut self, client_id: String) {
        let mut subscribers = self.subscribers.lock().unwrap();
        let _ = subscribers.unsubscribe_client(client_id);
    }

    async fn on_receive(&self, reader: &OwnedReadHalf, m: Message) {
        // println!("{:?}", m);
        if m.topic().starts_with(SYSTEM_TOPIC_PREFIX) {
            // TODO: spawn new task
            // drop the prefix
            let client_id = reader.peer_addr().unwrap().to_string();
            match m.topic().trim_start_matches(SYSTEM_TOPIC_PREFIX) {
                SUBSCRIBE_TOPIC => {
                    // Message value contains subscription pattern
                    println!(
                        "New subscription request: {:?}",
                        (client_id.to_string(), &m.value().as_str().unwrap())
                    );
                    // Wait for ownership of mutex lock
                    let mut subscribers = self.subscribers.lock().unwrap();

                    // Add new subscription entry to the subscriber tree
                    subscribers.subscribe(&m.value().as_str().unwrap(), client_id.to_string());
                }
                _ => (),
            }
        };
    }

    /// Listen for incoming connections
    ///
    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port)).await?;

        loop {
            let (stream, _) = listener.accept().await?;
            println!("Connection made");
            let (r, w) = stream.into_split();
            let mut reader = MessageReader::new(r);
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
}
