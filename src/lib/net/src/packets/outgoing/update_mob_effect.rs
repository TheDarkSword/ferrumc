//! Update Mob Effect packet: an effect applied or refreshed.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::registry_remap::NetworkMobEffect;

#[derive(NetEncode)]
#[packet(packet_id = "update_mob_effect", state = "play")]
pub struct UpdateMobEffect {
    pub entity_id: VarInt,
    pub effect: NetworkMobEffect,
    /// Level above the first, so zero is Strength I.
    pub amplifier: VarInt,
    /// Ticks remaining. A negative duration is one that does not run out.
    pub duration: VarInt,
    /// Ambient, visible, shows an icon, blends with the sky: one bit each, in that order.
    pub flags: u8,
}
