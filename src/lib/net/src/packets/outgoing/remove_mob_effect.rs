//! Remove Mob Effect packet.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::var_int::VarInt;

#[derive(NetEncode)]
#[packet(packet_id = "remove_mob_effect", state = "play")]
pub struct RemoveMobEffect {
    pub entity_id: VarInt,
    pub effect: VarInt,
}
