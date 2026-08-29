//! Change Difficulty packet, which only a player with permission to may send.

use ferrumc_macros::{packet, NetDecode};
use ferrumc_net_codec::net_types::var_int::VarInt;

#[derive(NetDecode, Debug)]
#[packet(packet_id = "change_difficulty", state = "play")]
pub struct ChangeDifficulty {
    pub difficulty: VarInt,
}
