// use clap::Parser;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use std::error::Error;
use tokio;


mod client;
mod datagram;
mod server;
mod subscription_tree;
use server::Server;

use client::{Client};
use datagram::Message;
use rmpv::{Value};


#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Optional name to operate on
    name: Option<String>,

    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the server
    Server {
        /// Port to listen on
        #[arg(short, long)]
        port: u16,
    },

    /// Run the test client, sending sample messages to specified port
    TestClient {
        /// address to send messages to
        #[arg(short, long)]
        address: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli: Cli = Cli::parse();

    match &cli.command {
        Some(Commands::Server { port }) => {
            let mut server = Server::new(*port).await?;
            server.run().await?;
        }
        Some(Commands::TestClient { address }) => {
            println!("Running test client...");
            let mut client = Client::new();

            // Connect to server at specified address
            let mut channel = client
                .connect(address.to_string(), vec![String::from("*")])
                .await;

            // Example: publish messages from separate tasks
            let channel_clone = channel.clone();
            let write_future = tokio::spawn(async move {
                loop {
                    channel_clone.publish(Message::new("test", Value::from("Publishing from a separate task"))).await;
                    // tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            });

            // Event handler
            let read_future = tokio::spawn(async move {
                while let Some(message) = channel.recv().await {
                    // println!("{message:?}");
                }
            });

            let (r, w) = tokio::join!(read_future, write_future);
            r.unwrap();
            w.unwrap();
        }
        None => {}
    };

    Ok(())
}
