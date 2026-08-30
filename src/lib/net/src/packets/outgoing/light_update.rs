//! Light Update packet: new light for a chunk the client already has.
//!
//! Clients do not work light out for themselves, so a torch placed after a chunk was sent stays
//! invisible to them until this arrives. It carries the same payload the chunk packet does.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_world::chunk::light::network::NetworkLightData;

#[derive(NetEncode)]
#[packet(packet_id = "light_update", state = "play")]
pub struct LightUpdate<'a> {
    pub chunk_x: VarInt,
    pub chunk_z: VarInt,
    pub light: NetworkLightData<'a>,
}
