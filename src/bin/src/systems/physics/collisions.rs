//! Stopping entities at the shapes blocks actually occupy.
//!
//! Movement is resolved one axis at a time against the boxes in the way, so an entity that walks
//! into a wall keeps the speed it had along the wall rather than stopping dead. The vertical axis
//! is settled first: standing on something has to be decided before sliding along it.
//!
//! Whether a block is in the way is its collision shape's business, not a list of names. A torch,
//! a carpet and a flower share a block position with whoever walks through them.

use bevy_ecs::change_detection::Ref;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{DetectChanges, Entity, Has, Query, Res, With};
use bevy_ecs::world::Mut;
use bevy_math::{DVec3, IVec3};
use ferrumc_core::transform::grounded::OnGround;
use ferrumc_core::transform::position::Position;
use ferrumc_core::transform::velocity::Velocity;
use ferrumc_entities::components::Baby;
use ferrumc_entities::entity_type::EntityType;
use ferrumc_entities::markers::HasCollisions;
use ferrumc_messages::entity_update::SendEntityUpdate;
use ferrumc_state::{GlobalState, GlobalStateResource};
use ferrumc_world::block_shape::{Aabb, VoxelShape};
use ferrumc_world::block_state::Axis;
use ferrumc_world::pos::{ChunkBlockPos, ChunkPos};

type CollisionQueryItem<'a> = (
    Entity,
    Mut<'a, Velocity>,
    Ref<'a, Position>,
    &'a EntityType,
    Has<Baby>,
    Mut<'a, OnGround>,
);

pub fn handle(
    query: Query<CollisionQueryItem, With<HasCollisions>>,
    mut writer: MessageWriter<SendEntityUpdate>,
    state: Res<GlobalStateResource>,
) {
    for (eid, mut vel, pos, kind, is_baby, mut grounded) in query {
        if !pos.is_changed() && !vel.is_changed() {
            continue;
        }
        let physical = kind.physical(is_baby);

        let movement = vel.as_dvec3();
        if movement == DVec3::ZERO {
            continue;
        }

        let entity = Aabb::new(
            pos.coords.x + f64::from(physical.bounding_box.min.x),
            pos.coords.y + f64::from(physical.bounding_box.min.y),
            pos.coords.z + f64::from(physical.bounding_box.min.z),
            pos.coords.x + f64::from(physical.bounding_box.max.x),
            pos.coords.y + f64::from(physical.bounding_box.max.y),
            pos.coords.z + f64::from(physical.bounding_box.max.z),
        );
        let allowed = collide(&state.0, entity, movement);
        if allowed == movement {
            continue;
        }

        // An entity that was moving down and is no longer is standing on something.
        if allowed.y != movement.y {
            grounded.0 = movement.y < 0.0;
            vel.y = 0.0;
        }
        if allowed.x != movement.x {
            vel.x = 0.0;
        }
        if allowed.z != movement.z {
            vel.z = 0.0;
        }

        writer.write(SendEntityUpdate(eid));
    }
}

/// How much of `movement` the world leaves an entity of `entity` size.
///
/// Axes are resolved one at a time and the box carried forward between them, so a move that is
/// blocked on one axis is still tried on the others. The vertical goes first, then the larger
/// horizontal, which is the order that lets an entity slide along a wall it is pressed into rather
/// than catching on the corner of every block.
fn collide(state: &GlobalState, entity: Aabb, movement: DVec3) -> DVec3 {
    let mut moving = entity;
    let mut allowed = DVec3::ZERO;

    allowed.y = axis(state, &moving, Axis::Y, movement.y);
    moving = moving.offset(0.0, allowed.y, 0.0);

    if movement.x.abs() >= movement.z.abs() {
        allowed.x = axis(state, &moving, Axis::X, movement.x);
        moving = moving.offset(allowed.x, 0.0, 0.0);
        allowed.z = axis(state, &moving, Axis::Z, movement.z);
    } else {
        allowed.z = axis(state, &moving, Axis::Z, movement.z);
        moving = moving.offset(0.0, 0.0, allowed.z);
        allowed.x = axis(state, &moving, Axis::X, movement.x);
    }

    allowed
}

/// How far the box may travel along one axis before a block shape stops it.
fn axis(state: &GlobalState, moving: &Aabb, axis: Axis, movement: f64) -> f64 {
    if movement == 0.0 {
        return 0.0;
    }

    let swept = swept(moving, axis, movement);
    let mut allowed = movement;
    for x in swept.min_x.floor() as i32..=swept.max_x.floor() as i32 {
        for y in swept.min_y.floor() as i32..=swept.max_y.floor() as i32 {
            for z in swept.min_z.floor() as i32..=swept.max_z.floor() as i32 {
                let block = IVec3::new(x, y, z);
                let shape = VoxelShape::collision_of(block_at(state, block));
                if shape.is_empty() {
                    continue;
                }
                allowed = shape.collide(
                    axis,
                    moving,
                    (f64::from(x), f64::from(y), f64::from(z)),
                    allowed,
                );
            }
        }
    }
    allowed
}

/// Everything the box passes through on its way, so a fast entity cannot step over a thin block.
fn swept(moving: &Aabb, axis: Axis, movement: f64) -> Aabb {
    let mut swept = *moving;
    match axis {
        Axis::X if movement > 0.0 => swept.max_x += movement,
        Axis::X => swept.min_x += movement,
        Axis::Y if movement > 0.0 => swept.max_y += movement,
        Axis::Y => swept.min_y += movement,
        Axis::Z if movement > 0.0 => swept.max_z += movement,
        Axis::Z => swept.min_z += movement,
    }
    swept
}

fn block_at(state: &GlobalState, pos: IVec3) -> ferrumc_world::block_state_id::BlockStateId {
    ferrumc_utils::world::load_or_generate_mut(state, ChunkPos::from(pos.as_dvec3()), "overworld")
        .expect("Failed to load or generate chunk")
        .get_block(ChunkBlockPos::from(pos))
}

/// Whether anything at all stops an entity at this position.
pub fn is_solid_block(state: &GlobalState, pos: IVec3) -> bool {
    !VoxelShape::collision_of(block_at(state, pos)).is_empty()
}
