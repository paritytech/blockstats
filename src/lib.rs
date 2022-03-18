use futures::{TryStream, TryStreamExt};
use std::{boxed::Box, fmt, sync::Arc};
use subxt::{
    rpc::RpcError, sp_runtime::traits::Header, ClientBuilder, DefaultConfig, DefaultExtra,
};

/// 50% of what is stored in configuration::activeConfig::maxPovSize at the relay chain.
const POV_MAX: u64 = 5_242_880 / 2;

#[subxt::subxt(runtime_metadata_path = "metadata/substrate.scale")]
pub mod substrate {}

type SubstrateRuntime = substrate::RuntimeApi<DefaultConfig, DefaultExtra<DefaultConfig>>;

pub struct BlockStats {
    number: u32,
    pov_len: u64,
    witness_len: u64,
    len: u64,
    weight: u64,
    num_extrinsics: u64,
    max_pov: u64,
    max_weight: u64,
}

impl fmt::Display for BlockStats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:04}: PoV Size={:04}KiB({:03}%) Weight={:07}µs({:03}%) Witness={:04}KiB Block={:04}KiB NumExtrinsics={:04}",
            self.number,
            self.pov_len / 1024,
            self.pov_len * 100 / self.max_pov,
            self.weight / 1_000_000_000,
            self.weight * 100 / self.max_weight,
            self.witness_len / 1024,
            self.len / 1024,
            self.num_extrinsics,
        )
    }
}

pub async fn subscribe_stats(
    url: &str,
) -> Result<impl TryStream<Ok = BlockStats, Error = RpcError> + Unpin, RpcError> {
    let api = Arc::new(
        ClientBuilder::new()
            .set_url(url)
            .build()
            .await
            .map_err(|_| RpcError::Custom("Failed to create client".to_string()))?
            .to_runtime_api::<SubstrateRuntime>(),
    );

    let max_weight = api.constants().system().block_weights().unwrap();
    let blocks = api
        .client
        .rpc()
        .subscribe_blocks()
        .await
        .map_err(|_| RpcError::Custom("Failed to subscribe to blocks".to_string()))?;

    Ok(Box::pin(blocks.and_then(move |block| {
        let api = api.clone();
        async move {
            let stats = api
                .client
                .rpc()
                .block_stats(block.hash())
                .await
                .map_err(|_| RpcError::Request("Failed to query block stats".to_string()))?
                .ok_or_else(|| RpcError::Request("Block not available.".to_string()))?;
            let weight = api
                .storage()
                .system()
                .block_weight(Some(block.hash()))
                .await
                .map_err(|_| RpcError::Request("Failed to query block weight".to_string()))?;
            let pov_len = stats.witness_len + stats.block_len;
            let total_weight = weight.normal + weight.operational + weight.mandatory;

            Ok(BlockStats {
                number: *block.number(),
                pov_len,
                witness_len: stats.witness_len,
                len: stats.block_len,
                weight: total_weight,
                num_extrinsics: stats.num_extrinsics,
                max_pov: POV_MAX,
                max_weight: max_weight.max_block,
            })
        }
    })))
}
