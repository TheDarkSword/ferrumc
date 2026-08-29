use ferrumc_inventories::slot::InventorySlot;
use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::var_int::VarInt;

#[derive(NetEncode)]
#[downgrade_with(crate::translate::to_1_21::container_set_slot)]
#[packet(packet_id = "container_set_slot", state = "play")]
pub struct SetContainerSlot {
    pub window_id: VarInt,
    pub state_id: VarInt,
    pub slot_index: i16,
    pub slot: InventorySlot,
}
