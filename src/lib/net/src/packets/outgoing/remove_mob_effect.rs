//! Remove Mob Effect packet.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::registry_remap::NetworkMobEffect;

#[derive(NetEncode)]
#[packet(packet_id = "remove_mob_effect", state = "play")]
pub struct RemoveMobEffect {
    pub entity_id: VarInt,
    pub effect: NetworkMobEffect,
}

impl RemoveMobEffect {
    #[must_use]
    pub const fn new(entity_id: i32, effect: u32) -> Self {
        Self {
            entity_id: VarInt::new(entity_id),
            effect: NetworkMobEffect(effect),
        }
    }
}
