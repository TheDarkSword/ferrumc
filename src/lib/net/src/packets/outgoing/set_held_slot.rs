use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::var_int::VarInt;

/// Server-to-Client packet to set the player's selected hotbar slot.
#[derive(NetEncode, Copy, Clone)]
#[downgrade_with(crate::translate::to_1_21_2::set_held_slot)]
#[packet(packet_id = "set_held_slot", state = "play")]
pub struct SetHeldItem {
    /// The hotbar slot to select (0-8).
    pub slot: VarInt,
}
