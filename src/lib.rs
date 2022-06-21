use futures::{TryStream, TryStreamExt};
use std::{boxed::Box, fmt, sync::Arc};
use subxt::{
    sp_core::H256, sp_runtime::traits::Header, BasicError, Client, ClientBuilder, DefaultConfig,
};

/// 50% of what is stored in configuration::activeConfig::maxPovSize at the relay chain.
const POV_MAX: u64 = 5_242_880 / 2;

#[derive(Debug)]
pub struct BlockStats {
    pub hash: H256,
    pub number: u32,
    pub pov_len: u64,
    pub witness_len: u64,
    pub len: u64,
    pub weight: u64,
    pub num_extrinsics: u64,
    pub max_pov: u64,
    pub max_weight: u64,
}

impl fmt::Display for BlockStats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:04}: PoV Size={:04}KiB({:03}%) Weight={:07}ms({:03}%) Witness={:04}KiB Block={:04}KiB NumExtrinsics={:04}",
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
) -> Result<impl TryStream<Ok = BlockStats, Error = BasicError> + Unpin, BasicError> {
    let client: Client<DefaultConfig> = ClientBuilder::new().set_url(url).build().await?;
    let client = Arc::new(client);

    let blocks = client
        .rpc()
        .subscribe_blocks()
        .await?
        .map_err(BasicError::from);

    let max_block_weights: BlockWeights = {
        let locked_metadata = client.metadata();
        let metadata = locked_metadata.read();
        let pallet = metadata.pallet("System")?;
        let constant = pallet.constant("BlockWeights")?;
        codec::Decode::decode(&mut &constant.value[..])?
    };

    Ok(Box::pin(blocks.map_err(Into::into).and_then(
        move |block| {
            let client = client.clone();
            let block_weight_storage_entry = BlockWeightStorageEntry;
            async move {
                let stats = client
                    .rpc()
                    .block_stats(block.hash())
                    .await?
                    .ok_or_else(|| BasicError::Other("Block not available.".to_string()))?;
                let weight = client
                    .storage()
                    .fetch_or_default(&block_weight_storage_entry, Some(block.hash()))
                    .await?;
                let pov_len = stats.witness_len + stats.block_len;
                let total_weight = weight.normal + weight.operational + weight.mandatory;

                Ok(BlockStats {
                    hash: block.hash(),
                    number: *block.number(),
                    pov_len,
                    witness_len: stats.witness_len,
                    len: stats.block_len,
                    weight: total_weight,
                    num_extrinsics: stats.num_extrinsics,
                    max_pov: POV_MAX,
                    max_weight: max_block_weights.max_block,
                })
            }
        },
    )))
}

#[derive(Clone)]
struct BlockWeightStorageEntry;

impl subxt::StorageEntry for BlockWeightStorageEntry {
    const PALLET: &'static str = "System";
    const STORAGE: &'static str = "BlockWeight";
    type Value = PerDispatchClass<u64>;
    fn key(&self) -> subxt::StorageEntryKey {
        subxt::StorageEntryKey::Plain
    }
}

#[derive(codec::Encode, codec::Decode)]
struct BlockWeights {
    pub base_block: u64,
    pub max_block: u64,
    pub per_class: PerDispatchClass<WeightsPerClass>,
}

#[derive(codec::Encode, codec::Decode)]
struct PerDispatchClass<T> {
    normal: T,
    operational: T,
    mandatory: T,
}

#[derive(codec::Encode, codec::Decode)]
pub struct WeightsPerClass {
    pub base_extrinsic: u64,
    pub max_extrinsic: Option<u64>,
    pub max_total: Option<u64>,
    pub reserved: Option<u64>,
}
