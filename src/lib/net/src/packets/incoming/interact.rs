//! Interact Entity packet.
//!
//! Sent when a player uses an entity. Attacking one is [`super::attack::AttackEntity`], which 26.1
//! split out of this packet; an older client sends both as an interaction with an action field,
//! and the translator sorts them out.

use ferrumc_macros::{packet, NetDecode};
use ferrumc_net_codec::net_types::lp_vec3::LowPrecisionVec3;
use ferrumc_net_codec::net_types::var_int::VarInt;

/// Sent when a player interacts with an entity.
#[derive(NetDecode, Debug)]
#[upgrade_with(crate::translate::to_1_21_11::interact)]
#[upgrade_into(crate::packets::incoming::attack::AttackEntity)]
#[packet(packet_id = "interact", state = "play")]
pub struct InteractEntity {
    /// The entity being interacted with.
    pub entity_id: VarInt,
    /// Which hand was used.
    pub hand: VarInt,
    /// Where on the entity, relative to it. Zero where the client did not aim at a point.
    pub location: LowPrecisionVec3,
    /// Whether the player was sneaking, which selects the secondary interaction.
    pub using_secondary_action: bool,
}
