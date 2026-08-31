//! What a tick does to an entity's velocity, before and after it moves.
//!
//! The arithmetic is in `ferrumc_physics`, which knows nothing about the world. All that happens
//! here is asking the world the two questions it needs — what an entity is standing on, and what it
//! is standing in — and applying the answer.
//!
//! Both halves run because vanilla pulls the two kinds of entity down at different points in the
//! tick. A mob moves with what the tick before left it and is pulled down afterwards; a dropped
//! thing is pulled down first and moves with that. Which of the two an entity follows is the
//! game's own answer, carried on the type.

use bevy_ecs::prelude::{Query, Res};
use bevy_math::{IVec3, Vec3A};
use ferrumc_core::transform::grounded::OnGround;
use ferrumc_core::transform::position::Position;
use ferrumc_core::transform::velocity::Velocity;
use ferrumc_entities::entity_type::EntityType;
use ferrumc_macros::match_block;
use ferrumc_physics::{after_move, before_move, Fluid, Footing, DEFAULT_BLOCK_FRICTION};
use ferrumc_state::{GlobalState, GlobalStateResource};
use ferrumc_world::block_state_id::BlockStateId;
use ferrumc_world::pos::{ChunkBlockPos, ChunkPos};

/// Pulls a dropped thing down before it moves. A mob moves with what the last tick left it.
pub fn pull_before_moving(
    mut entities: Query<(&mut Velocity, &Position, &EntityType)>,
    state: Res<GlobalStateResource>,
) {
    for (mut velocity, position, kind) in &mut entities {
        let motion = kind.motion();
        if motion.living || (motion.gravity == 0.0 && **velocity == Vec3A::ZERO) {
            continue;
        }
        **velocity = before_move(**velocity, motion, fluid_at(&state.0, position));
    }
}

/// Pulls a mob down and slows it, and slows what a dropped thing was left with.
pub fn pull_and_slow_after_moving(
    mut entities: Query<(&mut Velocity, &Position, &EntityType, &OnGround)>,
    state: Res<GlobalStateResource>,
) {
    for (mut velocity, position, kind, grounded) in &mut entities {
        let motion = kind.motion();
        if motion.gravity == 0.0 && **velocity == Vec3A::ZERO {
            continue;
        }
        let fluid = fluid_at(&state.0, position);
        **velocity = after_move(**velocity, motion, footing(grounded), fluid);
    }
}

/// What an entity is standing on.
///
/// Every block holds an entity back the same amount so far; ice and slime and honey do not, and
/// what each of them does is on the block rather than in any report.
fn footing(grounded: &OnGround) -> Footing {
    if grounded.0 {
        Footing::On(DEFAULT_BLOCK_FRICTION)
    } else {
        Footing::None
    }
}

/// What an entity is standing in, if anything.
///
/// Vanilla asks how deep the fluid is across the whole box; this asks what is at the entity's feet,
/// which is the same answer everywhere but at the very edge of a pool.
fn fluid_at(state: &GlobalState, position: &Position) -> Option<Fluid> {
    let feet = position.coords.as_ivec3();
    let block = block_at(state, feet);
    if match_block!("water", block) {
        Some(Fluid::Water)
    } else if match_block!("lava", block) {
        Some(Fluid::Lava)
    } else {
        None
    }
}

fn block_at(state: &GlobalState, pos: IVec3) -> BlockStateId {
    ferrumc_utils::world::load_or_generate_mut(state, ChunkPos::from(pos.as_dvec3()), "overworld")
        .expect("Failed to load or generate chunk")
        .get_block(ChunkBlockPos::from(pos))
}
