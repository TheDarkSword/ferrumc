//! Player Rotation packet: turns the player where the server wants them looking.

use ferrumc_macros::{packet, NetEncode};

#[derive(NetEncode)]
#[packet(packet_id = "player_rotation", state = "play")]
pub struct PlayerRotation {
    pub yaw: f32,
    /// Whether the angle is added to where the player is looking rather than replacing it.
    pub relative_yaw: bool,
    pub pitch: f32,
    pub relative_pitch: bool,
}
