//! Change Difficulty packet, which also says whether a player may change it back.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::var_int::VarInt;

#[derive(NetEncode)]
#[downgrade_with(crate::translate::to_1_21_5::change_difficulty)]
#[packet(packet_id = "change_difficulty", state = "play")]
pub struct ChangeDifficulty {
    pub difficulty: VarInt,
    pub locked: bool,
}
