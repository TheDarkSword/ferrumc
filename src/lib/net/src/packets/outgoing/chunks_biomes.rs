//! Chunks Biomes packet: new biomes for chunks the client already has, without resending them.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::byte_array::ByteArray;
use ferrumc_net_codec::net_types::length_prefixed_vec::LengthPrefixedVec;

#[derive(NetEncode)]
pub struct ChunkBiomes {
    pub chunk_x: i32,
    pub chunk_z: i32,
    /// Every section's biome container, one after another, as they appear in a chunk packet.
    pub data: ByteArray,
}

#[derive(NetEncode)]
#[packet(packet_id = "chunks_biomes", state = "play")]
pub struct ChunksBiomes {
    pub chunks: LengthPrefixedVec<ChunkBiomes>,
}
