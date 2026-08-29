//! Mount Screen Open packet: the inventory of a ridden animal, which has no menu type of its own.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::var_int::VarInt;

#[derive(NetEncode)]
#[packet(packet_id = "mount_screen_open", state = "play")]
pub struct MountScreenOpen {
    pub window_id: VarInt,
    /// Columns of storage the animal carries, which depends on what it is wearing.
    pub inventory_columns: VarInt,
    pub entity_id: i32,
}
