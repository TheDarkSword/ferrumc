//! Set Cursor Item packet: what a player is holding on the pointer while a container is open.

use ferrumc_inventories::slot::InventorySlot;
use ferrumc_macros::{packet, NetEncode};

#[derive(NetEncode)]
#[packet(packet_id = "set_cursor_item", state = "play")]
pub struct SetCursorItem {
    pub item: InventorySlot,
}
