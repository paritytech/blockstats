use clap::Parser;
use subxt::{sp_runtime::traits::Header, ClientBuilder, DefaultConfig, DefaultExtra};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// The node to connect to.
    #[clap(long, default_value = "ws://localhost:9944/")]
    url: String,
}

#[subxt::subxt(runtime_metadata_path = "metadata/substrate.scale")]
pub mod substrate {}

type SubstrateRuntime = substrate::RuntimeApi<DefaultConfig, DefaultExtra<DefaultConfig>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let api = ClientBuilder::new()
        .set_url(args.url)
        .build()
        .await?
        .to_runtime_api::<SubstrateRuntime>();

    let mut blocks = api.client.rpc().subscribe_blocks().await?;

    while let Some(Ok(block)) = blocks.next().await {
        let stats = api.client.rpc().block_stats(Some(block.hash())).await?;
        println!("{}: {:?}", block.number(), stats);
    }

    Ok(())
}
