//! Set Entity Motion packet: a push, such as an explosion or knockback.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::lp_vec3::LowPrecisionVec3;
use ferrumc_net_codec::net_types::var_int::VarInt;

#[derive(NetEncode)]
#[downgrade_with(crate::translate::to_1_21_7::set_entity_motion)]
#[packet(packet_id = "set_entity_motion", state = "play")]
pub struct SetEntityMotion {
    pub entity_id: VarInt,
    /// Blocks a tick, packed.
    pub velocity: LowPrecisionVec3,
}
