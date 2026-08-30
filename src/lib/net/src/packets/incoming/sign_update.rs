//! Sign Update packet: what a player typed into a sign.

use ferrumc_macros::{packet, NetDecode};
use ferrumc_net_codec::net_types::network_position::NetworkPosition;

#[derive(NetDecode, Debug)]
#[packet(packet_id = "sign_update", state = "play")]
pub struct SignUpdate {
    pub position: NetworkPosition,
    /// Which face was written on.
    pub is_front: bool,
    pub line_1: String,
    pub line_2: String,
    pub line_3: String,
    pub line_4: String,
}
