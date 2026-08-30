use crate::errors::NetError;
use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::length_prefixed_vec::LengthPrefixedVec;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::version::ProtocolVersion;
use ferrumc_world::chunk::light::network::NetworkLightData;
use ferrumc_world::chunk::network::NetworkChunk;
use ferrumc_world::chunk::Chunk;
use ferrumc_world::pos::ChunkPos;

#[derive(NetEncode)]
pub struct BlockEntity {
    pub xz: u8,
    pub y: i16,
    pub entity_type: VarInt,
    pub nbt: Vec<u8>,
}

#[derive(NetEncode)]
pub struct NetHeightmap {
    // Define the structure of your heightmaps here
    pub id: VarInt,
    pub data: LengthPrefixedVec<i64>,
}

#[derive(NetEncode)]
#[packet(packet_id = "level_chunk_with_light", state = "play")]
#[downgrade_with(crate::translate::to_1_21_4::level_chunk_with_light)]
pub struct ChunkAndLightData<'chunk> {
    pub chunk_x: i32,
    pub chunk_z: i32,
    // The binary nbt data
    pub chunk_data: NetworkChunk,
    pub block_entities: LengthPrefixedVec<BlockEntity>,
    pub light_data: NetworkLightData<'chunk>,
}

impl<'chunk> ChunkAndLightData<'chunk> {
    pub fn from_chunk(
        pos: ChunkPos,
        chunk: &'chunk Chunk,
        version: ProtocolVersion,
    ) -> Result<Self, NetError> {
        Ok(ChunkAndLightData::<'chunk> {
            chunk_x: pos.x(),
            chunk_z: pos.z(),
            chunk_data: NetworkChunk::new(chunk, version)?,
            block_entities: LengthPrefixedVec::new(
                chunk
                    .block_entities()
                    .iter()
                    .map(|entity| BlockEntity {
                        // The two horizontal coordinates share a byte, four bits each.
                        xz: (entity.x << 4) | (entity.z & 0x0F),
                        y: entity.y,
                        entity_type: VarInt::new(i32::from(entity.kind)),
                        nbt: entity.to_nbt(),
                    })
                    .collect(),
            ),
            light_data: NetworkLightData::from(chunk),
        })
    }
}
