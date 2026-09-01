//! Telling a player they have died, and what killed them.
//!
//! This is what puts the death screen up. Without it a client at zero health simply stands there
//! with an empty health bar and no way to come back.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_nbt::NBT;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_text::TextComponent;

#[derive(NetEncode, Debug, Clone)]
#[packet(packet_id = "player_combat_kill", state = "play")]
pub struct PlayerCombatKillPacket {
    pub player_id: VarInt,
    pub message: NBT<TextComponent>,
}

impl PlayerCombatKillPacket {
    #[must_use]
    pub fn new(player_id: i32, message: TextComponent) -> Self {
        Self {
            player_id: VarInt::new(player_id),
            message: NBT::new(message),
        }
    }
}
