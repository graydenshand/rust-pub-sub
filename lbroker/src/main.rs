//! lbroker command line interface
//! It can be used to run the server, test client, and log server metrics.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use env_logger;
use lbroker::buffer_config::{BufferConfig, FlushStrategy};
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

        /// Performance mode preset (low-latency, balanced, high-throughput)
        #[arg(long, value_name = "MODE")]
        mode: Option<String>,

        /// Custom buffer size in KB (overrides mode preset)
        #[arg(long, value_name = "KB")]
        buffer_size: Option<usize>,

        /// Flush strategy: immediate, auto, or periodic (overrides mode preset)
        #[arg(long, value_name = "STRATEGY")]
        flush_strategy: Option<String>,

        /// Flush interval in milliseconds (only for periodic strategy)
        #[arg(long, value_name = "MS", default_value = "10")]
        flush_interval: u64,
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
        Some(Commands::Server {
            port,
            mode,
            buffer_size,
            flush_strategy,
            flush_interval,
        }) => {
            // Determine buffer configuration
            let buffer_config = if let Some(mode_str) = mode {
                match mode_str.to_lowercase().as_str() {
                    "low-latency" | "lowlatency" => {
                        info!("Using low-latency mode (4KB buffer, immediate flush)");
                        BufferConfig::low_latency()
                    }
                    "balanced" => {
                        info!("Using balanced mode (32KB buffer, 10ms periodic flush)");
                        BufferConfig::balanced()
                    }
                    "high-throughput" | "highthroughput" => {
                        info!("Using high-throughput mode (128KB buffer, auto flush)");
                        BufferConfig::high_throughput()
                    }
                    _ => {
                        eprintln!("Unknown mode '{}', using balanced", mode_str);
                        BufferConfig::balanced()
                    }
                }
            } else if buffer_size.is_some() || flush_strategy.is_some() {
                // Custom configuration
                let size = buffer_size.map(|kb| kb * 1024).unwrap_or(32 * 1024);
                let strategy = if let Some(strat_str) = flush_strategy {
                    match strat_str.to_lowercase().as_str() {
                        "immediate" => FlushStrategy::Immediate,
                        "auto" => FlushStrategy::Auto,
                        "periodic" => FlushStrategy::Periodic {
                            interval_ms: *flush_interval,
                        },
                        _ => {
                            eprintln!("Unknown flush strategy '{}', using auto", strat_str);
                            FlushStrategy::Auto
                        }
                    }
                } else {
                    FlushStrategy::Auto
                };
                info!(
                    "Using custom buffer config ({}KB buffer, {:?} flush)",
                    size / 1024,
                    strategy
                );
                BufferConfig::custom(size, strategy)
            } else {
                // Default
                info!("Using default balanced mode (32KB buffer, 10ms periodic flush)");
                BufferConfig::default()
            };

            info!("Listening on port {port}");
            let mut server = Server::new(*port, buffer_config).await?;
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
