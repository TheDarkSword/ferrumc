//! Clear Dialog packet: closes whatever dialog the server opened. Carries nothing.

use ferrumc_macros::{packet, NetEncode};

#[derive(NetEncode)]
#[packet(packet_id = "clear_dialog", state = "play")]
pub struct ClearDialog;
