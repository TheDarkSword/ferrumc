//! Putting mobs in the world, and taking them away again.
//!
//! The questions are in `ferrumc_spawning`, which knows nothing about the world. All that happens
//! here is answering them from the chunks players are keeping loaded, and turning what comes back
//! into entities.
//!
//! Vanilla tries this every tick for every loaded chunk, and almost every attempt fails on its
//! first question. That is what makes the world feel populated rather than crowded, so the same
//! shape is kept rather than a cheaper one that spawns more.

use bevy_ecs::prelude::*;
use ferrumc_core::chunks::chunk_receiver::ChunkReceiver;
use ferrumc_core::identity::player_identity::PlayerIdentity;
use ferrumc_core::transform::position::Position;
use ferrumc_entities::entity_type::{EntityType, MobCategory};
use ferrumc_messages::entity_spawn::SpawnEntityEvent;
use ferrumc_spawning::{should_despawn, spawn_in_chunk, Pos, SpawnState, SpawnWorld, Spawner};
use ferrumc_state::{GlobalState, GlobalStateResource};
use ferrumc_world::block_state_id::BlockStateId;
use ferrumc_world::pos::{ChunkBlockPos, ChunkPos};
use std::collections::HashMap;

/// Which categories are tried, which is every one the game counts as a mob.
const SPAWNING_CATEGORIES: [MobCategory; 7] = [
    MobCategory::Monster,
    MobCategory::Creature,
    MobCategory::Ambient,
    MobCategory::Axolotls,
    MobCategory::UndergroundWaterCreature,
    MobCategory::WaterCreature,
    MobCategory::WaterAmbient,
];

/// What lives in each biome, by the number the chunk carries.
///
/// Built once from the packs: a chunk records a biome as a registry number, and what lives there is
/// written against the biome's name.
#[derive(Resource, Default)]
pub struct BiomeSpawns {
    by_biome: HashMap<u8, HashMap<MobCategory, Vec<Spawner>>>,
}

impl BiomeSpawns {
    /// Reads what every biome the packs define says lives in it.
    #[must_use]
    pub fn load(worldgen: &ferrumc_worldgen_data::WorldgenData) -> Self {
        let mut by_biome: HashMap<u8, HashMap<MobCategory, Vec<Spawner>>> = HashMap::new();
        for (name, biome) in &worldgen.biomes {
            let Some(id) = ferrumc_registry::tags::protocol_id("minecraft:worldgen/biome", name)
                .and_then(|id| u8::try_from(id).ok())
            else {
                continue;
            };
            let mut categories: HashMap<MobCategory, Vec<Spawner>> = HashMap::new();
            for (category, spawners) in &biome.spawners {
                let Some(category) = MobCategory::from_name(category) else {
                    continue;
                };
                let listed = spawners
                    .iter()
                    .filter_map(|spawner| {
                        Some(Spawner {
                            kind: EntityType::from_name(&spawner.entity.to_string())?,
                            weight: u32::try_from(spawner.weight).ok()?,
                            min_count: u32::try_from(spawner.min_count).unwrap_or(1).max(1),
                            max_count: u32::try_from(spawner.max_count).unwrap_or(1).max(1),
                        })
                    })
                    .collect::<Vec<_>>();
                if !listed.is_empty() {
                    categories.insert(category, listed);
                }
            }
            by_biome.insert(id, categories);
        }
        Self { by_biome }
    }

    fn at(&self, biome: u8, category: MobCategory) -> &[Spawner] {
        self.by_biome
            .get(&biome)
            .and_then(|categories| categories.get(&category))
            .map_or(&[], Vec::as_slice)
    }
}

/// The world as the spawn questions want to ask it, ready to be asked.
pub fn world_around<'a>(
    state: &'a GlobalState,
    spawns: &'a BiomeSpawns,
    players: &'a [(f64, f64, f64)],
) -> impl SpawnWorld + 'a {
    Around {
        state,
        spawns,
        players,
        surfaces: std::cell::RefCell::default(),
    }
}

/// The world as the spawn questions want to ask it.
struct Around<'a> {
    state: &'a GlobalState,
    spawns: &'a BiomeSpawns,
    players: &'a [(f64, f64, f64)],
    /// How high the ground is in each column asked about. A chunk this server generated carries no
    /// heightmap, so the answer is found by looking down the column, and the spawn loop asks for
    /// the same column once per category.
    surfaces: std::cell::RefCell<HashMap<(i32, i32), i32>>,
}

impl Around<'_> {
    fn chunk_of(&self, pos: Pos) -> Option<ferrumc_world::MutChunk<'_>> {
        ferrumc_utils::world::load_or_generate_mut(
            self.state,
            ChunkPos::from(pos.as_dvec3()),
            "overworld",
        )
        .ok()
    }
}

impl SpawnWorld for Around<'_> {
    fn block_at(&self, pos: Pos) -> BlockStateId {
        self.chunk_of(pos)
            .map_or_else(BlockStateId::default, |chunk| {
                chunk.get_block(ChunkBlockPos::from(pos))
            })
    }

    fn block_light(&self, pos: Pos) -> u8 {
        self.chunk_of(pos)
            .map_or(0, |chunk| chunk.block_light(ChunkBlockPos::from(pos)))
    }

    fn sky_light(&self, pos: Pos) -> u8 {
        self.chunk_of(pos)
            .map_or(0, |chunk| chunk.sky_light(ChunkBlockPos::from(pos)))
    }

    fn can_see_sky(&self, pos: Pos) -> bool {
        self.surface_at(pos.x, pos.z) <= pos.y
    }

    fn brightness(&self, pos: Pos) -> u8 {
        self.block_light(pos).max(self.sky_light(pos))
    }

    fn spawners_at(&self, pos: Pos, category: MobCategory) -> &[Spawner] {
        let biome = self
            .chunk_of(pos)
            .map_or(0, |chunk| chunk.get_biome(ChunkBlockPos::from(pos)).0);
        self.spawns.at(biome, category)
    }

    fn surface_at(&self, x: i32, z: i32) -> i32 {
        if let Some(known) = self.surfaces.borrow().get(&(x, z)) {
            return *known;
        }
        let found = self.chunk_of(Pos::new(x, 0, z)).map_or(0, |chunk| {
            let local = ChunkBlockPos::from(Pos::new(x, 0, z));
            chunk.surface_height(local.x(), local.z())
        });
        self.surfaces.borrow_mut().insert((x, z), found);
        found
    }

    fn min_y(&self) -> i32 {
        -64
    }

    fn nearest_player_sqr(&self, x: f64, y: f64, z: f64) -> Option<f64> {
        self.players
            .iter()
            .map(|(px, py, pz)| {
                let (dx, dy, dz) = (px - x, py - y, pz - z);
                dx * dx + dy * dy + dz * dz
            })
            .min_by(f64::total_cmp)
    }

    fn fits(&self, kind: EntityType, x: f64, y: f64, z: f64) -> bool {
        // Whether the box the mob would occupy is clear. Its own block was already checked, so
        // this is about how tall it is.
        let (_, height) = kind.size();
        let top = (y + f64::from(height)).ceil() as i32;
        (y.floor() as i32..top)
            .all(|level| !self.is_solid(Pos::new(x.floor() as i32, level, z.floor() as i32)))
    }

    fn standable(&self, pos: Pos) -> bool {
        ferrumc_world::block_data::valid_spawn(self.block_at(pos))
    }

    fn is_water(&self, pos: Pos) -> bool {
        ferrumc_macros::match_block!("water", self.block_at(pos))
    }

    fn is_solid(&self, pos: Pos) -> bool {
        !ferrumc_world::block_shape::VoxelShape::collision_of(self.block_at(pos)).is_empty()
    }
}

/// Tries to put mobs somewhere in the chunks players are keeping loaded.
pub fn spawn_mobs(
    players: Query<(&Position, &ChunkReceiver), With<PlayerIdentity>>,
    mobs: Query<(&EntityType, &Position), Without<PlayerIdentity>>,
    spawns: Res<BiomeSpawns>,
    state: Res<GlobalStateResource>,
    mut events: MessageWriter<SpawnEntityEvent>,
) {
    let mut loaded: Vec<(i32, i32)> = Vec::new();
    let mut where_players_are: Vec<(f64, f64, f64)> = Vec::new();
    for (position, receiver) in &players {
        where_players_are.push((position.x, position.y, position.z));
        loaded.extend(receiver.loaded.iter().copied());
    }
    if loaded.is_empty() {
        return;
    }
    loaded.sort_unstable();
    loaded.dedup();

    let mut counts = SpawnState::new(u32::try_from(loaded.len()).unwrap_or(u32::MAX));
    for (kind, position) in &mobs {
        let chunk = ChunkPos::from(position.coords);
        counts.count(kind.category(), chunk.pos.x, chunk.pos.y);
    }

    let mut rng = rand::thread_rng();
    let world = world_around(&state.0, &spawns, &where_players_are);

    for category in SPAWNING_CATEGORIES {
        if !counts.has_room_in_the_world(category) {
            continue;
        }
        for &(x, z) in &loaded {
            if !counts.has_room_here(category, x, z) {
                continue;
            }
            for put in spawn_in_chunk(&world, &mut counts, category, x, z, &mut rng) {
                events.write(SpawnEntityEvent::fresh(
                    put.kind,
                    Position::from(bevy_math::DVec3::new(put.x, put.y, put.z)),
                ));
            }
        }
    }
}

/// Takes away mobs nobody is near.
pub fn despawn_mobs(
    mobs: Query<(Entity, &EntityType, &Position), Without<PlayerIdentity>>,
    players: Query<&Position, With<PlayerIdentity>>,
    mut commands: Commands,
) {
    let where_players_are: Vec<&Position> = players.iter().collect();
    let mut rng = rand::thread_rng();

    for (entity, kind, position) in &mobs {
        if kind.category() == MobCategory::Misc {
            continue;
        }
        let nearest = where_players_are
            .iter()
            .map(|player| {
                let (dx, dy, dz) = (
                    player.x - position.x,
                    player.y - position.y,
                    player.z - position.z,
                );
                dx * dx + dy * dy + dz * dz
            })
            .min_by(f64::total_cmp);

        // Nothing asks a mob to stay yet, so whether it was asked is always no.
        if should_despawn(*kind, nearest, false, &mut rng) {
            commands.entity(entity).despawn();
        }
    }
}
