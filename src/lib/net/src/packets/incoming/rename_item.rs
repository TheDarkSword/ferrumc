//! Rename Item packet: what a player typed into an anvil.

use ferrumc_macros::{packet, NetDecode};

#[derive(NetDecode, Debug)]
#[packet(packet_id = "rename_item", state = "play")]
pub struct RenameItem {
    pub name: String,
}
