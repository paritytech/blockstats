use clap::Parser;
use futures::{StreamExt, TryStreamExt};

/// Subscribe to new blocks of a chain and print stats about each block.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// The node to connect to. Needs to be a websocket.
    #[clap(long, default_value = "ws://localhost:9944/")]
    url: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut stats = blockstats::subscribe_stats(&args.url).await?.into_stream();

    while let Some(stat) = stats.next().await {
        println!("{}", stat?);
    }

    Ok(())
}
