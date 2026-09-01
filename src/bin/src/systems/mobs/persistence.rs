//! Keeping entities between one run of the server and the next.
//!
//! An entity belongs to the chunk it stands in. A chunk that comes into view brings back whatever
//! was standing in it, and one that nobody is near any more is written out and its entities let go
//! of — which is also how a chunk is given the animals it is born with, since a chunk arriving for
//! the first time has none saved and has never been populated.
//!
//! Players are not entities for this purpose: they are saved by their own name when they leave.

use bevy_ecs::prelude::*;
use ferrumc_core::chunks::chunk_receiver::ChunkReceiver;
use ferrumc_core::identity::entity_identity::EntityIdentity;
use ferrumc_core::identity::player_identity::PlayerIdentity;
use ferrumc_core::transform::grounded::OnGround;
use ferrumc_core::transform::position::Position;
use ferrumc_core::transform::rotation::Rotation;
use ferrumc_core::transform::velocity::Velocity;
use ferrumc_entities::entity_type::EntityType;
use ferrumc_messages::entity_spawn::SpawnEntityEvent;
use ferrumc_spawning::populate_new_chunk;
use ferrumc_state::GlobalStateResource;
use ferrumc_world::entities::SavedEntity;
use ferrumc_world::pos::ChunkPos;
use std::collections::{HashMap, HashSet};

use super::natural::{world_around, BiomeSpawns};

/// What is read off an entity to write it down. Everything about where it is and how it is moving,
/// which is all a saved entity carries.
type Written<'a> = (
    &'a EntityType,
    &'a EntityIdentity,
    &'a Position,
    &'a Rotation,
    &'a Velocity,
    &'a OnGround,
);

/// Which chunks have their entities in the world rather than on disk.
#[derive(Resource, Default)]
pub struct LiveChunks {
    live: HashSet<(i32, i32)>,
}

/// Brings back what was standing in a chunk that has just come into view, and gives a chunk that
/// has never been seen before the animals it is born with.
pub fn load_entities_for_new_chunks(
    players: Query<&ChunkReceiver, With<PlayerIdentity>>,
    spawns: Res<BiomeSpawns>,
    state: Res<GlobalStateResource>,
    mut live: ResMut<LiveChunks>,
    mut events: MessageWriter<SpawnEntityEvent>,
) {
    let mut wanted: HashSet<(i32, i32)> = HashSet::new();
    for receiver in &players {
        wanted.extend(receiver.loaded.iter().copied());
    }

    let mut rng = rand::thread_rng();
    for &(x, z) in wanted.difference(&live.live.clone()) {
        let at = ChunkPos::new(x, z);
        for saved in state
            .0
            .world
            .load_entities(at, "overworld")
            .into_iter()
            .flatten()
        {
            let Some(kind) = EntityType::from_protocol_id(saved.kind) else {
                continue;
            };
            events.write(SpawnEntityEvent {
                entity_type: kind,
                position: bevy_math::DVec3::from(saved.position).into(),
                uuid: Some(uuid::Uuid::from_u128(saved.uuid)),
            });
        }

        // A chunk being seen for the first time is given its animals here rather than when it was
        // generated, because that happens off the tick thread where nothing can be spawned.
        let populated = state
            .0
            .world
            .cached_chunk(at, "overworld")
            .is_none_or(|chunk| chunk.populated());
        if !populated {
            let world = world_around(&state.0, &spawns, &[]);
            for put in populate_new_chunk(&world, x, z, creature_probability(), &mut rng) {
                events.write(SpawnEntityEvent::fresh(
                    put.kind,
                    bevy_math::DVec3::new(put.x, put.y, put.z).into(),
                ));
            }
            if let Some(mut chunk) = state.0.world.cached_chunk_mut(at, "overworld") {
                chunk.mark_populated();
            }
        }

        live.live.insert((x, z));
    }
}

/// Writes out and lets go of the entities in chunks nobody is near any more.
pub fn unload_entities_for_gone_chunks(
    players: Query<&ChunkReceiver, With<PlayerIdentity>>,
    mobs: Query<(Entity, Written), Without<PlayerIdentity>>,
    state: Res<GlobalStateResource>,
    mut live: ResMut<LiveChunks>,
    mut commands: Commands,
) {
    let mut wanted: HashSet<(i32, i32)> = HashSet::new();
    for receiver in &players {
        wanted.extend(receiver.loaded.iter().copied());
    }
    let going: HashSet<(i32, i32)> = live.live.difference(&wanted).copied().collect();
    if going.is_empty() {
        return;
    }

    let mut by_chunk: HashMap<(i32, i32), Vec<SavedEntity>> = HashMap::new();
    let mut leaving = Vec::new();
    for (entity, (kind, identity, position, rotation, velocity, grounded)) in &mobs {
        let chunk = ChunkPos::from(position.coords);
        let at = (chunk.pos.x, chunk.pos.y);
        if !going.contains(&at) {
            continue;
        }
        by_chunk.entry(at).or_default().push(SavedEntity {
            kind: kind.protocol_id(),
            uuid: identity.uuid.as_u128(),
            position: [position.x, position.y, position.z],
            rotation: [rotation.yaw, rotation.pitch],
            velocity: velocity.to_array(),
            on_ground: grounded.0,
        });
        leaving.push(entity);
    }

    for at in going {
        let held = by_chunk.remove(&at).unwrap_or_default();
        if let Err(err) = state
            .0
            .world
            .save_entities(ChunkPos::new(at.0, at.1), "overworld", &held)
        {
            tracing::error!("could not write the entities in {at:?}: {err}");
            continue;
        }
        live.live.remove(&at);
    }
    for entity in leaving {
        commands.entity(entity).despawn();
    }
}

/// Writes out every entity that is loaded, without letting go of any.
///
/// The world is written to disk on a timer so that a crash costs little; entities are written with
/// it for the same reason.
pub fn save_all_entities(
    mobs: Query<Written, Without<PlayerIdentity>>,
    live: Res<LiveChunks>,
    state: Res<GlobalStateResource>,
) {
    let mut by_chunk: HashMap<(i32, i32), Vec<SavedEntity>> = HashMap::new();
    for (kind, identity, position, rotation, velocity, grounded) in &mobs {
        let chunk = ChunkPos::from(position.coords);
        by_chunk
            .entry((chunk.pos.x, chunk.pos.y))
            .or_default()
            .push(SavedEntity {
                kind: kind.protocol_id(),
                uuid: identity.uuid.as_u128(),
                position: [position.x, position.y, position.z],
                rotation: [rotation.yaw, rotation.pitch],
                velocity: velocity.to_array(),
                on_ground: grounded.0,
            });
    }

    // Every live chunk is written, including the ones that emptied: a chunk whose last mob wandered
    // off has to stop holding it.
    for &at in &live.live {
        let held = by_chunk.remove(&at).unwrap_or_default();
        if let Err(err) = state
            .0
            .world
            .save_entities(ChunkPos::new(at.0, at.1), "overworld", &held)
        {
            tracing::error!("could not write the entities in {at:?}: {err}");
        }
    }
}

/// How readily a chunk is given animals when it is first seen.
///
/// The biome carries its own number and nothing reads it yet, so this is the one vanilla uses for
/// almost every biome.
const fn creature_probability() -> f32 {
    0.1
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::message::MessageRegistry;
    use bevy_math::{DVec3, Vec3A};
    use ferrumc_state::create_test_state;

    /// A world with one mob in it, in a chunk a player has loaded.
    ///
    /// The temporary directory comes back with it: it holds the world on disk, and dropping it
    /// early would take the world with it.
    fn a_world_with_a_zombie() -> (World, Entity, GlobalStateResource, tempfile::TempDir) {
        let mut world = World::new();
        let (state, temp) = create_test_state();
        world.insert_resource(state.clone());
        world.insert_resource(LiveChunks {
            live: [(0, 0)].into_iter().collect(),
        });
        MessageRegistry::register_message::<SpawnEntityEvent>(&mut world);

        let zombie = world
            .spawn((
                EntityIdentity::new(),
                EntityType::Zombie,
                Position::from(DVec3::new(8.5, 64.0, 8.5)),
                Rotation::default(),
                Velocity { vec: Vec3A::ZERO },
                OnGround(true),
            ))
            .id();
        (world, zombie, state, temp)
    }

    #[test]
    fn a_mob_in_a_chunk_nobody_is_near_is_written_out_and_let_go_of() {
        let (mut world, zombie, state, _held) = a_world_with_a_zombie();

        // No players at all, so every live chunk is one nobody is near.
        let mut schedule = Schedule::default();
        schedule.add_systems(unload_entities_for_gone_chunks);
        schedule.run(&mut world);

        assert!(
            world.get_entity(zombie).is_err(),
            "the mob should have been let go of"
        );
        let written = state
            .0
            .world
            .load_entities(ChunkPos::new(0, 0), "overworld")
            .expect("reading the chunk back");
        assert_eq!(written.len(), 1, "and written down");
        assert_eq!(written[0].kind, EntityType::Zombie.protocol_id());
        assert_eq!(written[0].position, [8.5, 64.0, 8.5]);
    }

    #[test]
    fn a_chunk_that_emptied_stops_holding_anything() {
        let (mut world, zombie, state, _held) = a_world_with_a_zombie();
        state
            .0
            .world
            .save_entities(
                ChunkPos::new(0, 0),
                "overworld",
                &[SavedEntity {
                    kind: EntityType::Zombie.protocol_id(),
                    uuid: 1,
                    position: [8.5, 64.0, 8.5],
                    rotation: [0.0, 0.0],
                    velocity: [0.0; 3],
                    on_ground: true,
                }],
            )
            .expect("writing one to begin with");
        world.despawn(zombie);

        let mut schedule = Schedule::default();
        schedule.add_systems(save_all_entities);
        schedule.run(&mut world);

        assert!(
            state
                .0
                .world
                .load_entities(ChunkPos::new(0, 0), "overworld")
                .expect("reading it back")
                .is_empty(),
            "a chunk whose last mob went should not still be holding it"
        );
    }

    #[test]
    fn a_mob_that_comes_back_keeps_the_name_it_had() {
        let (mut world, zombie, state, _held) = a_world_with_a_zombie();
        let was = world
            .get::<EntityIdentity>(zombie)
            .expect("it has a name")
            .uuid;

        let mut out = Schedule::default();
        out.add_systems(unload_entities_for_gone_chunks);
        out.run(&mut world);

        let written = state
            .0
            .world
            .load_entities(ChunkPos::new(0, 0), "overworld")
            .expect("reading it back");
        assert_eq!(
            written[0].uuid,
            was.as_u128(),
            "the name is what makes it the same mob rather than another like it"
        );
    }
}
