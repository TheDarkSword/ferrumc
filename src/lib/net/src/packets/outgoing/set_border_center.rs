//! Set Border Center packet.

use ferrumc_macros::{packet, NetEncode};

#[derive(NetEncode)]
#[packet(packet_id = "set_border_center", state = "play")]
pub struct SetBorderCenter {
    pub center_x: f64,
    pub center_z: f64,
}
