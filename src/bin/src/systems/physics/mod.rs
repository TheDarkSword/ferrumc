use bevy_ecs::schedule::IntoScheduleConfigs;
pub mod collisions;
pub mod motion;
pub mod velocity;

/// The order a tick moves an entity in, which is the order vanilla moves one in.
///
/// A dropped thing is pulled down, the world shortens what is left of the move, the move happens,
/// and everything is slowed afterwards — a mob being pulled down at that point too, since it moves
/// with what the tick before left it. Getting this order wrong does not look wrong: everything
/// still falls, only a couple of blocks off over the first second.
pub fn register_physics(schedule: &mut bevy_ecs::schedule::Schedule) {
    schedule.add_systems(
        (
            motion::pull_before_moving,
            collisions::handle,
            velocity::handle,
            motion::pull_and_slow_after_moving,
        )
            .chain(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::*;
    use bevy_math::DVec3;
    use ferrumc_core::identity::entity_identity::EntityIdentity;
    use ferrumc_core::transform::grounded::OnGround;
    use ferrumc_core::transform::position::Position;
    use ferrumc_core::transform::velocity::Velocity;
    use ferrumc_entities::entity_type::EntityType;
    use ferrumc_entities::markers::HasCollisions;
    use ferrumc_macros::block;
    use ferrumc_state::{create_test_state, GlobalStateResource};
    use ferrumc_world::block_state_id::BlockStateId;
    use ferrumc_world::pos::{ChunkBlockPos, ChunkPos};

    /// A world with a floor at y = 0 and nothing above it.
    fn floor(state: &GlobalStateResource) {
        let mut chunk =
            ferrumc_utils::world::load_or_generate_mut(&state.0, ChunkPos::new(0, 0), "overworld")
                .expect("a chunk to put a floor in");
        chunk.fill(block!("air"));
        for x in 0..16 {
            for z in 0..16 {
                chunk.set_block(ChunkBlockPos::new(x, 0, z), block!("stone"));
            }
        }
    }

    /// Runs `ticks` of the whole pipeline over one entity, and says where it ended up.
    fn drop_from(kind: EntityType, height: f64, ticks: usize) -> (DVec3, bool) {
        let mut world = World::new();
        let (state, _temp) = create_test_state();
        floor(&state);
        world.insert_resource(state);

        let entity = world
            .spawn((
                EntityIdentity::new(),
                kind,
                Position::from(DVec3::new(8.0, height, 8.0)),
                Velocity::zero(),
                OnGround(false),
                HasCollisions,
            ))
            .id();

        let mut schedule = Schedule::default();
        register_physics(&mut schedule);
        for _ in 0..ticks {
            schedule.run(&mut world);
        }

        let position = world.get::<Position>(entity).expect("it is still there");
        let grounded = world.get::<OnGround>(entity).expect("it is still there");
        (position.coords, grounded.0)
    }

    #[test]
    fn a_dropped_item_falls_lands_and_stays_on_the_floor() {
        let (landed, grounded) = drop_from(EntityType::Item, 10.0, 200);
        assert!(grounded, "it should be standing on the floor");
        assert!(
            landed.y >= 1.0,
            "it should be on the floor, not in it: {landed:?}"
        );
        assert!(
            landed.y < 1.05,
            "and on it rather than hovering above it: {landed:?}"
        );
    }

    #[test]
    fn a_mob_falls_faster_than_an_item() {
        let (item, _) = drop_from(EntityType::Item, 200.0, 20);
        let (zombie, _) = drop_from(EntityType::Zombie, 200.0, 20);
        assert!(
            zombie.y < item.y,
            "a mob is pulled down twice as hard: mob {}, item {}",
            zombie.y,
            item.y
        );
    }

    #[test]
    fn a_fall_covers_the_ground_vanilla_covers() {
        // One second of falling from rest: long enough for a wrong order to show, short enough to
        // still be accelerating. These are the game's own numbers.
        let (zombie, _) = drop_from(EntityType::Zombie, 500.0, 20);
        let (item, _) = drop_from(EntityType::Item, 500.0, 20);
        assert!(
            (500.0 - zombie.y - 13.2512).abs() < 0.01,
            "a mob fell {}",
            500.0 - zombie.y
        );
        assert!(
            (500.0 - item.y - 7.4256).abs() < 0.01,
            "an item fell {}",
            500.0 - item.y
        );
    }

    #[test]
    fn a_thing_that_is_never_pulled_down_stays_where_it_is_put() {
        let (painting, _) = drop_from(EntityType::Painting, 10.0, 100);
        assert_eq!(painting.y, 10.0);
    }
}
