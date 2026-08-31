//! What a player asks to start or stop doing.
//!
//! Sprinting is the part of it that everyone else can see; the rest of the actions belong to
//! systems that do not exist yet.

use bevy_ecs::prelude::{Query, Res};
use ferrumc_entities::synced_data::{EntityFlag, SyncedData};
use ferrumc_net::packets::incoming::player_command::PlayerCommandAction;
use ferrumc_net::PlayerCommandPacketReceiver;

pub fn handle(receiver: Res<PlayerCommandPacketReceiver>, mut players: Query<&mut SyncedData>) {
    for (event, eid) in receiver.0.try_iter() {
        let sprinting = match event.action {
            PlayerCommandAction::StartSprinting => true,
            PlayerCommandAction::StopSprinting => false,
            _ => continue,
        };
        let Ok(mut data) = players.get_mut(eid) else {
            continue;
        };
        data.set_flag(EntityFlag::Sprinting, sprinting);
    }
}
