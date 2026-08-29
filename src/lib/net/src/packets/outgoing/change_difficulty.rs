//! Change Difficulty packet, which also says whether a player may change it back.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::var_int::VarInt;

#[derive(NetEncode)]
#[packet(packet_id = "change_difficulty", state = "play")]
pub struct ChangeDifficulty {
    pub difficulty: VarInt,
    pub locked: bool,
}
