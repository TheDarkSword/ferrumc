//! Set Border Lerp Size packet: the border starts moving to a new size.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::var_long::VarLong;

#[derive(NetEncode)]
#[downgrade_with(crate::translate::to_1_21_9::set_border_lerp_size)]
#[packet(packet_id = "set_border_lerp_size", state = "play")]
pub struct SetBorderLerpSize {
    pub old_diameter: f64,
    pub new_diameter: f64,
    /// Milliseconds the move takes.
    pub speed: VarLong,
}
