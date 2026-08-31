//! What a player's own client says it is doing with the keys.
//!
//! The one thing read out of it here is whether the player is crouching, which reaches everyone
//! else as a bit of the shared flags byte and as a pose. Since 1.21 it arrives in this packet
//! rather than in a player command, which is where it used to be.

use bevy_ecs::prelude::{Query, Res};
use ferrumc_entities::synced_data::{EntityFlag, SyncedData};
use ferrumc_net::PlayerInputReceiver;

/// The bit this packet uses to say the player is holding the sneak key.
const SNEAKING: u8 = 0x20;

pub fn handle(receiver: Res<PlayerInputReceiver>, mut players: Query<&mut SyncedData>) {
    for (event, eid) in receiver.0.try_iter() {
        let Ok(mut data) = players.get_mut(eid) else {
            continue;
        };
        data.set_flag(EntityFlag::Crouching, event.flags & SNEAKING != 0);
    }
}
