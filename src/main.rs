use subxt::{ClientBuilder, DefaultConfig, DefaultExtra, sp_runtime::traits::Header};

#[subxt::subxt(runtime_metadata_path = "metadata/canvas.scale")]
pub mod canvas {}

type CanvasRuntime = canvas::RuntimeApi<DefaultConfig, DefaultExtra<DefaultConfig>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api = ClientBuilder::new()
        .set_url("wss://canvas-rococo-rpc.polkadot.io:443")
        .build()
        .await?
        .to_runtime_api::<CanvasRuntime>();

	let mut blocks = api.client.rpc().subscribe_blocks().await?;

	while let Some(Ok(block)) = blocks.next().await {
		println!("Block: {}", block.number());
	}

	Ok(())
}
