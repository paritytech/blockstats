//! Connect to a parachain RPC node and monitor stats about its blocks.
//! This includes the PoV (witness vs. transactions), weight and TX
//! pool fullness. This is useful to gain insights where about bottlenecks
//! (computationb vs bandwith).

use core::ops::Add;
use futures::{TryStream, TryStreamExt};
use std::{boxed::Box, fmt};
use subxt::{
    ext::{scale_decode, sp_core::H256},
    storage::{address::StaticStorageMapKey, address::Yes, Address},
    Error, OnlineClient, PolkadotConfig as DefaultConfig, error::MetadataError, backend::{legacy::{LegacyRpcMethods}, rpc::RpcClient}
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
    pub weight: Weight,
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
    pub max_weight: Weight,
}

impl fmt::Display for BlockStats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:04}: PoV Size={:04}KiB({:03}%) Weight RefTime={:07}ms({:03}%) Weight ProofSize={:04}KiB({:03}%) Witness={:04}KiB Block={:04}KiB NumExtrinsics={:04}",
            self.number,
            self.pov_len / 1024,
            self.pov_len * 100 / self.max_pov,
            self.weight.ref_time / 1_000_000_000,
            self.weight.ref_time * 100 / self.max_weight.ref_time,
            self.weight.proof_size / 1024,
            self.weight.proof_size * 100 / self.max_weight.proof_size,
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
    let rpc_client = RpcClient::from_url(url).await?;
    subscribe_stats_with_client(rpc_client).await
}

/// Connect to the specified node and listen for new blocks using OnlineClient.
pub async fn subscribe_stats_with_client(
    rpc_client: RpcClient,
) -> Result<impl TryStream<Ok = BlockStats, Error = Error> + Unpin, Error> {
    let client = OnlineClient::<DefaultConfig>::from_rpc_client(rpc_client.clone()).await?;
    let blocks = client.blocks().subscribe_best().await?;

    let max_block_weights: BlockWeights = {
        let metadata = client.metadata();
        let pallet = metadata.pallet_by_name_err("System")?;
        let constant_name = "BlockWeights";
        let constant = pallet
            .constant_by_name(constant_name)
            .ok_or_else(|| {
                MetadataError::ConstantNameNotFound(constant_name.to_owned())
            })?;
        codec::Decode::decode(&mut &constant.value()[..])?
    };

    Ok(Box::pin(blocks.map_err(Into::into).and_then(
        move |block| {
            let client = client.clone();
            let rpc_methods = LegacyRpcMethods::<DefaultConfig>::new(rpc_client.clone());

            let block_weight_address =
                Address::<StaticStorageMapKey, PerDispatchClass<Weight>, Yes, Yes, ()>::new_static(
                    "System",
                    "BlockWeight",
                    vec![],
                    Default::default(),
                )
                .unvalidated();
            async move {
                let stats = rpc_methods
                    .dev_get_block_stats(block.hash())
                    .await?
                    .ok_or_else(|| Error::Other("Block not available.".to_string()))?;
                let weight = client
                    .storage()
                    .at(block.hash())
                    .fetch_or_default(&block_weight_address)
                    .await?;
                let pov_len = stats.witness_len + stats.block_len;
                let total_weight = weight.normal + weight.operational + weight.mandatory;

                Ok(BlockStats {
                    hash: block.hash(),
                    number: block.number(),
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

/// Copied from `sp_weight` to additionally implement `scale_decode::DecodeAsType`.
#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Debug,
    Default,
    codec::Encode,
    codec::Decode,
    codec::MaxEncodedLen,
    scale_decode::DecodeAsType,
)]
#[decode_as_type(crate_path = "scale_decode")]
pub struct Weight {
    #[codec(compact)]
    /// The weight of computational time used based on some reference hardware.
    ref_time: u64,
    #[codec(compact)]
    /// The weight of storage space used by proof of validity.
    proof_size: u64,
}

impl Add for Weight {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            ref_time: self.ref_time + rhs.ref_time,
            proof_size: self.proof_size + rhs.proof_size,
        }
    }
}

#[derive(codec::Decode, codec::Encode, scale_decode::DecodeAsType)]
#[decode_as_type(crate_path = "scale_decode")]
struct BlockWeights {
    pub base_block: Weight,
    pub max_block: Weight,
    pub per_class: PerDispatchClass<WeightsPerClass>,
}

#[derive(codec::Decode, codec::Encode, scale_decode::DecodeAsType)]
#[decode_as_type(crate_path = "scale_decode")]
struct PerDispatchClass<T> {
    normal: T,
    operational: T,
    mandatory: T,
}

#[derive(codec::Decode, codec::Encode, scale_decode::DecodeAsType)]
#[decode_as_type(crate_path = "scale_decode")]
struct WeightsPerClass {
    pub base_extrinsic: Weight,
    pub max_extrinsic: Option<Weight>,
    pub max_total: Option<Weight>,
    pub reserved: Option<Weight>,
}
