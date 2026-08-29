//! Move Player (status only) packet: sent every tick a player neither moves nor turns.

use ferrumc_macros::{packet, NetDecode};

#[derive(NetDecode, Debug)]
#[packet(packet_id = "move_player_status_only", state = "play")]
pub struct MovePlayerStatusOnly {
    /// Bit 0 is standing on the ground, bit 1 is pushing against a wall.
    pub flags: u8,
}
