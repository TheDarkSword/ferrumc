//! Move Vehicle packet: where the vehicle a player is steering has got to.

use ferrumc_macros::{packet, NetDecode};

#[derive(NetDecode, Debug)]
#[packet(packet_id = "move_vehicle", state = "play")]
pub struct MoveVehicle {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}
