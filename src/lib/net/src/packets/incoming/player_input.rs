use ferrumc_macros::{packet, NetDecode};

#[derive(NetDecode)]
#[upgrade_with(crate::translate::to_1_21::player_input)]
#[packet(packet_id = "player_input", state = "play")]
pub struct PlayerInput {
    pub flags: u8,
}
