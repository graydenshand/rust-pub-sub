use std::error::Error;
use tokio;
use futures::future::join;
use clap::Parser;

mod datagram;
mod subscription_tree;
mod gsub;
use gsub::GSub;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Port to listen on
    #[arg(short, long)]
    port: u16,

    /// Path to subscriptions file
    #[arg(short, long)]
    subscriptions: String
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let gsub = GSub::new(args.port, &args.subscriptions).await?;
    
    gsub.run().await;
    Ok(())
}
