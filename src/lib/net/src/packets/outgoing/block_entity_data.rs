//! Block Entity Data packet: what a block holds, for a block the client already has.
//!
//! Sent when a sign is written on or a block entity otherwise changes. The batch that comes with a
//! chunk covers everything that was there when it was sent.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::network_position::NetworkPosition;
use ferrumc_net_codec::net_types::var_int::VarInt;

#[derive(NetEncode)]
#[packet(packet_id = "block_entity_data", state = "play")]
pub struct BlockEntityData {
    pub position: NetworkPosition,
    /// Which kind, as the registry id.
    pub entity_type: VarInt,
    /// The block entity's own fields, already serialised.
    pub nbt: Vec<u8>,
}
