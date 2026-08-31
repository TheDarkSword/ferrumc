//! Whether a player is swimming, and what that makes them look like.

use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use ferrumc_core::transform::position::Position;
use ferrumc_entities::synced_data::{EntityFlag, SyncedData};
use ferrumc_macros::match_block;
use ferrumc_state::GlobalStateResource;
use ferrumc_world::block_state_id::BlockStateId;
use ferrumc_world::pos::BlockPos;

/// How far above their feet a player's eyes are, in blocks.
const EYE_HEIGHT: f64 = 1.62;

/// Whether a player's head is in water.
fn head_in_water(state: &ferrumc_state::GlobalState, pos: &Position) -> bool {
    let eyes = DVec3::new(pos.x, pos.y + EYE_HEIGHT, pos.z)
        .floor()
        .as_ivec3();
    state
        .world
        .get_block_and_fetch(BlockPos::of(eyes.x, eyes.y, eyes.z), "overworld")
        .map(|block| match_block!("water", block))
        .unwrap_or(false)
}

/// Marks players who are in water, so that everyone else sees them swim.
///
/// Nothing is broadcast from here: writing the flag is what makes it a change, and one system
/// sends every change an entity has accumulated at the end of the tick.
pub fn detect_player_swimming(
    mut players: Query<(&Position, &mut SyncedData)>,
    state: Res<GlobalStateResource>,
) {
    for (position, mut data) in &mut players {
        data.set_flag(EntityFlag::Swimming, head_in_water(&state.0, position));
    }
}
