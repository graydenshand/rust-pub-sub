// use clap::Parser;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use std::error::Error;
use tokio;
use tokio::sync::mpsc;

mod client;
mod datagram;
mod server;
mod subscription_tree;
use server::Server;

use client::{Client, Subscription};
use datagram::Message;
use rmpv::{Value, Utf8String};


// #[derive(Parser, Debug)]
// #[command(version, about, long_about = None)]
// struct Args {
    /// Port to listen on
    // #[arg(short, long)]
    // port: u16,
// }

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
            let server = Server::new(*port).await?;
            server.run().await?;
        },
        Some(Commands::TestClient{ address }) => {
            println!("Running test client...");
            let (tx, mut rx) = mpsc::channel(32);
            let mut client = Client::new();
            client.connect(address.to_string(), vec![String::from("*")], tx).await;
            while let Some(message) = rx.recv().await {
                println!("{message:?}")
            };
            // Add subscription to echo messages sent
            // client.subscribe(address, "*").await?;
            // client.run(|message| {
            //     println!("{message:?}");
            // }).await
            
            // Receive messages
            // tokio::spawn(async move {
            //     client.run(|message| {
            //         println!("{message:?}");
            //     }).await
            // });

            // Publish messages            
            // tokio::spawn(async move {
            //     loop {
            //         for i in 0..10 {
            //             // messages.push(Message::new("test", Value::from(i)))
            //             client.publish(address, Message::new("test", Value::from(i))).await;
            //         }
            //         tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            //     }
            // });
        }
        None => {}
    };
    
    Ok(())
}
