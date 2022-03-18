use clap::Parser;
use futures::{StreamExt, TryStreamExt};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// The node to connect to.
    #[clap(long, default_value = "ws://localhost:9944/")]
    url: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut stats = povstats::subscribe_stats(&args.url).await?.into_stream();

    while let Some(stat) = stats.next().await {
        println!("{}", stat?);
    }

    Ok(())
}
