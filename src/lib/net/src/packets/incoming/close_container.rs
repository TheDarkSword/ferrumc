use ferrumc_macros::{packet, NetDecode};
use ferrumc_net_codec::net_types::var_int::VarInt;

#[derive(NetDecode)]
#[upgrade_with(crate::translate::to_1_21::container_close)]
#[packet(packet_id = "container_close", state = "play")]
pub struct CloseContainer {
    pub window_id: VarInt,
}
