//! lbroker command line interface
//! It can be used to run the server, test client, and log server metrics.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use env_logger;
use lbroker::config;
use log::info;
use std::error::Error;
use tokio;

use lbroker::client::{test_client, Client};
use lbroker::server::Server;

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

        /// Number of test clients to run
        #[arg(short, long)]
        number: u64,

        /// Pattern to subscribe to
        #[arg(short, long, action = clap::ArgAction::Append)]
        subscribe: Vec<String>,

        /// Interval in seconds at which each client should send messages
        #[arg(short, long)]
        interval: Option<f64>,
    },

    /// Log metrics published by the server
    LogMetrics {
        /// address to send messages to
        #[arg(short, long)]
        address: String,
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
        Some(Commands::TestClient {
            address,
            number,
            subscribe,
            interval,
        }) => {
            info!("Running test client...");
            let mut futures = vec![];
            for i in 0..*number {
                let ac = address.clone();
                let subscribers = subscribe.clone();
                let t_interval = match interval {
                    Some(i) => Some(tokio::time::interval(tokio::time::Duration::from_secs_f64(
                        *i,
                    ))),
                    None => None,
                };
                futures.push(tokio::spawn(async move {
                    test_client(&format!("{i}"), &ac, &subscribers[..], t_interval).await;
                }));
            }
            for f in futures {
                f.await.unwrap();
            }
        }
        Some(Commands::LogMetrics { address }) => {
            let mut client = Client::new(address.to_string()).await;
            client
                .subscribe(&format!("{}/metrics/*", config::SYSTEM_TOPIC_PREFIX))
                .await;
            while let Some(m) = client.recv().await {
                info!("{} - {}", m.topic, m.value.as_f64().unwrap())
            }
        }
        None => {}
    };

    Ok(())
}
