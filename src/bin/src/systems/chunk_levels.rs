//! Working out how much attention each chunk gets.
//!
//! Every tick the tickets are put back from where the players are, and the levels they spread to
//! decide what happens: what ticks, what is merely kept and sendable, and what is let go.
//!
//! Rebuilding rather than tracking changes is deliberate for now: a handful of players is a
//! handful of tickets, and a level that is always derived cannot drift from where the players
//! actually are.

use bevy_ecs::prelude::{Query, ResMut, Resource, With};
use ferrumc_config::server_config::get_global_config;
use ferrumc_core::transform::position::Position;
use ferrumc_net::connection::StreamWriter;
use ferrumc_world::chunk_level::ChunkLevels;

/// What every chunk is worth this tick.
#[derive(Resource, Default)]
pub struct Levels(pub ChunkLevels);

pub fn handle(mut levels: ResMut<Levels>, players: Query<&Position, With<StreamWriter>>) {
    let config = get_global_config();
    levels.0.clear();
    for position in players.iter() {
        let chunk = position.chunk();
        levels.0.add_player(
            ferrumc_world::pos::ChunkPos::new(chunk.x, chunk.y),
            config.chunk_render_distance,
            config.simulation_distance,
        );
    }
    levels.0.recompute();
}
