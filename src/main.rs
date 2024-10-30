// use clap::Parser;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use env_logger;
use log::{debug, info};
use std::error::Error;
use tokio;

use rmpv::Value;
use rps::client::Client;
use rps::server::Server;

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

        /// client_id server will use to trace this session
        #[arg(short, long, default_value_t = String::from("test-client"))]
        client_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli: Cli = Cli::parse();
    let log_level = match cli.debug {
        0 => "info",
        _ => "debug",
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    match &cli.command {
        Some(Commands::Server { port }) => {
            info!("Listening on port {port}");
            let mut server = Server::new(*port).await?;
            server.run().await?;
        }
        Some(Commands::TestClient { address, client_id }) => {
            info!("Running test client...");
            let mut client = Client::new(address.to_string(), client_id.to_string()).await;
            client.subscribe("*").await;

            let client_clone = client.clone();
            let write_future = tokio::spawn(async move {
                loop {
                    client_clone
                        .publish("test", Value::from("Publishing from a separate task"))
                        .await;
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                }
            });

            // Event handlers
            let read_future = tokio::spawn(async move {
                while let Some(message) = client.recv(None).await {
                    let topic = message.topic();
                    let value = message.value().to_string();
                    debug!("Message received - {topic} - {value}");
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
