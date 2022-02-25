use subxt::{sp_runtime::traits::Header, ClientBuilder, DefaultConfig, DefaultExtra};

#[subxt::subxt(runtime_metadata_path = "metadata/substrate.scale")]
pub mod substrate {}

type SubstrateRuntime = substrate::RuntimeApi<DefaultConfig, DefaultExtra<DefaultConfig>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api = ClientBuilder::new()
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
