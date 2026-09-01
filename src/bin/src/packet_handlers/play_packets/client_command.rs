//! Coming back after dying.
//!
//! A client at zero health shows the death screen and waits. Pressing respawn sends this, and what
//! it asks for is the whole of coming back: full health, empty lungs refilled, nothing still
//! burning, and a place to stand.

use bevy_ecs::prelude::*;
use ferrumc_components::health::Health;
use ferrumc_components::player::gamemode::GameModeComponent;
use ferrumc_components::player::hunger::Hunger;
use ferrumc_damage::{Reeling, Vitals};
use ferrumc_messages::teleport_player::TeleportPlayer;
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::incoming::client_command::ClientCommandAction;
use ferrumc_net::packets::outgoing::respawn::RespawnPacket;
use ferrumc_net::packets::outgoing::set_health::SetHealth;
use ferrumc_net::ClientCommandReceiver;
use ferrumc_state::GlobalStateResource;
use ferrumc_world::pos::{ChunkBlockPos, ChunkPos};
use tracing::warn;

/// Where a player comes back, until a world has a spawn point of its own to come back to.
const WORLD_SPAWN: (f64, f64) = (0.5, 0.5);

/// How far above the ground a player is put, so they land rather than stand inside it.
const ABOVE_THE_GROUND: f64 = 1.0;

/// What is written back to a player who has come back.
type ComingBack<'a> = (
    &'a StreamWriter,
    &'a GameModeComponent,
    &'a mut Health,
    &'a mut Hunger,
    &'a mut Vitals,
    &'a mut Reeling,
);

pub fn handle(
    commands: Res<ClientCommandReceiver>,
    mut players: Query<ComingBack>,
    state: Res<GlobalStateResource>,
    mut teleports: MessageWriter<TeleportPlayer>,
) {
    for (packet, player) in commands.0.try_iter() {
        if packet.action != ClientCommandAction::PerformRespawn {
            continue;
        }
        let Ok((writer, gamemode, mut health, mut hunger, mut vitals, mut reeling)) =
            players.get_mut(player)
        else {
            continue;
        };

        health.current = health.max;
        *hunger = Hunger::default();
        *vitals = Vitals::default();
        *reeling = Reeling::default();

        if let Err(err) = writer.send_packet_ref(&RespawnPacket::same_dimension(gamemode.0)) {
            warn!("could not put a player back in the world: {err:?}");
            continue;
        }
        if let Err(err) = writer.send_packet_ref(&SetHealth::new(
            health.current,
            i32::from(hunger.level),
            hunger.saturation,
        )) {
            warn!("could not tell a player they are well again: {err:?}");
        }

        let (x, z) = WORLD_SPAWN;
        let y = f64::from(ground_at(&state.0, x, z)) + ABOVE_THE_GROUND;
        vitals.last_y = y;
        teleports.write(TeleportPlayer {
            entity: player,
            x,
            y,
            z,
            vel_x: 0.0,
            vel_y: 0.0,
            vel_z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
        });
    }
}

/// How high the ground is at a place, so a player comes back on top of it rather than inside it.
fn ground_at(state: &ferrumc_state::GlobalState, x: f64, z: f64) -> i32 {
    let at = bevy_math::DVec3::new(x, 0.0, z);
    let local = ChunkBlockPos::from(at.as_ivec3());
    ferrumc_utils::world::load_or_generate_mut(state, ChunkPos::from(at), "overworld")
        .map_or(64, |chunk| chunk.surface_height(local.x(), local.z()))
}
