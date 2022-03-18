use clap::Parser;
use subxt::{sp_runtime::traits::Header, ClientBuilder, DefaultConfig, DefaultExtra};

/// 50% of what is stored in configuration::activeConfig::maxPovSize at the relay chain.
const POV_MAX: u64 = 5_242_880 / 2;

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

    let max_weight = api.constants().system().block_weights()?;

    let mut blocks = api.client.rpc().subscribe_blocks().await?;

    while let Some(Ok(block)) = blocks.next().await {
        let stats = api
            .client
            .rpc()
            .block_stats(block.hash())
            .await?
            .ok_or("Blub")?;
        let weight = api
            .storage()
            .system()
            .block_weight(Some(block.hash()))
            .await?;
        let pov_size = stats.witness_len + stats.block_len;
        let pov_fullness = pov_size * 100 / POV_MAX;
        let total_weight = weight.normal + weight.operational + weight.mandatory;
        let weight_fullness = total_weight * 100 / max_weight.max_block;

        println!(
            "{:04}: PoV Size={:04}KiB({:03}%) Weight={:07}µs({:03}%) Witness={:04}KiB Block={:04}KiB NumExtrinsics={:04}",
            block.number(),
            pov_size / 1024,
            pov_fullness,
            total_weight / 1_000_000_000,
            weight_fullness,
            stats.witness_len / 1024,
            stats.block_len / 1024,
            stats.num_extrinsics,
        );
    }

    Ok(())
}
