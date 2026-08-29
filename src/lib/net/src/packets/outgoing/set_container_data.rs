//! Container Set Data packet: one numbered property of an open container, such as a furnace's fuel.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::var_int::VarInt;

#[derive(NetEncode)]
#[packet(packet_id = "container_set_data", state = "play")]
pub struct SetContainerData {
    pub window_id: VarInt,
    /// Which property, numbered per container kind.
    pub property: i16,
    pub value: i16,
}
