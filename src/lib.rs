//! Connect to a parachain RPC node and monitor stats about its blocks.
//! This includes the PoV (witness vs. transactions), weight and TX
//! pool fullness. This is useful to gain insights where about bottlenecks
//! (computationb vs bandwith).

use futures::{TryStream, TryStreamExt};
use std::{boxed::Box, fmt, sync::Arc};
use subxt::{
    ext::{sp_core::H256, sp_runtime::traits::Header},
    metadata::DecodeStaticType,
    storage::{address::Yes, StaticStorageAddress},
    Error, OnlineClient, PolkadotConfig as DefaultConfig,
};

/// 50% of what is stored in configuration::activeConfig::maxPovSize at the relay chain.
const POV_MAX: u64 = 5_242_880 / 2;

/// Statistics regarding a specific block.
///
/// Use the custom [`fmt::Display`] implementation to pretty print it.
#[derive(Debug)]
pub struct BlockStats {
    /// The block hash.
    pub hash: H256,
    /// The block number.
    pub number: u32,
    /// Total length of the PoV.
    ///
    /// PoV is the complete data that is send by the collator to the relay chain validator.
    /// In case of cumulus based chains this includes the storage proof and the block itself.
    pub pov_len: u64,
    /// Size of the storage proof in bytes.
    pub witness_len: u64,
    /// Size of the block in bytes.
    pub len: u64,
    /// Overall weight used by the block.
    pub weight: u64,
    /// Number of extrinsics in a block.
    pub num_extrinsics: u64,
    /// The maximum allowed PoV size.
    ///
    /// Please note that this value is hardcoded to the value that is currently configured
    /// value in polkadot. It is stored in the `configuration::activeConfig::maxPovSize`
    /// storage item of the relay chain.
    pub max_pov: u64,
    /// The maximum allowed weight.
    ///
    /// Please note that this is the overall weight disregarding any weight classes. It
    /// is usually never reached even in a chain that is at capacity.
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

/// Connect to the specified node and listen for new blocks.
///
/// The `url` needs to be a websocket so that we can subscribe to new blocks.
pub async fn subscribe_stats(
    url: &str,
) -> Result<impl TryStream<Ok = BlockStats, Error = Error> + Unpin, Error> {
    let client = OnlineClient::<DefaultConfig>::from_url(url).await?;
    let client = Arc::new(client);

    let blocks = client.rpc().subscribe_blocks().await?.map_err(Error::from);

    let max_block_weights: BlockWeights = {
        let metadata = client.metadata();
        let pallet = metadata.pallet("System")?;
        let constant = pallet.constant("BlockWeights")?;
        codec::Decode::decode(&mut &constant.value[..])?
    };

    Ok(Box::pin(blocks.map_err(Into::into).and_then(
        move |block| {
            let client = client.clone();
            let block_weight_address =
                StaticStorageAddress::<DecodeStaticType<PerDispatchClass<u64>>, Yes, Yes, ()>::new(
                    "System",
                    "BlockWeight",
                    vec![],
                    Default::default(),
                )
                .unvalidated();
            async move {
                let stats = client
                    .rpc()
                    .block_stats(block.hash())
                    .await?
                    .ok_or_else(|| Error::Other("Block not available.".to_string()))?;
                let weight: PerDispatchClass<u64> = client
                    .storage()
                    .fetch_or_default(&block_weight_address, Some(block.hash()))
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
struct WeightsPerClass {
    pub base_extrinsic: u64,
    pub max_extrinsic: Option<u64>,
    pub max_total: Option<u64>,
    pub reserved: Option<u64>,
}
