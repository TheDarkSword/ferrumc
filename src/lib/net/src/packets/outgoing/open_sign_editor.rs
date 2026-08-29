//! Open Sign Editor packet: opens the text entry for a sign just placed or clicked.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::network_position::NetworkPosition;

#[derive(NetEncode)]
#[packet(packet_id = "open_sign_editor", state = "play")]
pub struct OpenSignEditor {
    pub position: NetworkPosition,
    /// Whether the front of the sign is being edited rather than the back.
    pub is_front_text: bool,
}
