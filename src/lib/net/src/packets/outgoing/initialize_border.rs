//! Initialize Border packet: the whole world border at once, sent on join.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::net_types::var_long::VarLong;

#[derive(NetEncode)]
#[packet(packet_id = "initialize_border", state = "play")]
pub struct InitializeBorder {
    pub center_x: f64,
    pub center_z: f64,
    /// Where the border is now, and where it is heading. Equal when it is not moving.
    pub old_diameter: f64,
    pub new_diameter: f64,
    /// Milliseconds the move takes. Zero when the border is already where it is going.
    pub speed: VarLong,
    pub portal_teleport_boundary: VarInt,
    /// How far out the warning haze starts, and how many seconds ahead of a closing border.
    pub warning_blocks: VarInt,
    pub warning_time: VarInt,
}
