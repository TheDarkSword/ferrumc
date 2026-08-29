//! Attack Entity packet.
//!
//! Split out of the interact packet in 26.1; before that an attack was an interaction with an
//! action of its own.

use ferrumc_macros::{packet, NetDecode};
use ferrumc_net_codec::net_types::var_int::VarInt;

/// Sent when a player hits another entity.
#[derive(NetDecode, Debug)]
#[packet(packet_id = "attack", state = "play")]
pub struct AttackEntity {
    /// The entity being hit.
    pub entity_id: VarInt,
}
