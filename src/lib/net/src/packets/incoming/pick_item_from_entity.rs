//! Pick Item From Entity packet: a middle click on an entity, in creative.

use ferrumc_macros::{packet, NetDecode};
use ferrumc_net_codec::net_types::var_int::VarInt;

#[derive(NetDecode, Debug)]
#[packet(packet_id = "pick_item_from_entity", state = "play")]
pub struct PickItemFromEntity {
    pub entity_id: VarInt,
    /// Whether the picked item should carry the entity's data as well as its kind.
    pub include_data: bool,
}
