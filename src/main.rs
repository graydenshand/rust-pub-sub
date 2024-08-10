use clap::Parser;

use std::error::Error;
use tokio;

mod client;
mod datagram;
mod server;
mod subscription_tree;
use server::Server;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Port to listen on
    #[arg(short, long)]
    port: u16,

    /// Path to subscriptions file
    #[arg(short, long)]
    subscriptions: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let Server = Server::new(args.port).await?;

    Server.run().await;
    Ok(())
}
