//! Where a mob appears on its own.
//!
//! A world only feels alive because the game is quietly trying, every tick, to put a mob somewhere
//! nobody is looking. It picks a position in a loaded chunk, asks the biome what lives there, and
//! then asks a long series of questions — is it dark enough, is there something to stand on, is a
//! player far enough away, is there already a crowd — and almost always the answer is no.
//!
//! Nothing here touches the world or the entity store. What the world has to say is a
//! [`SpawnWorld`], and what comes out is a list of mobs to put down, which the caller does. That
//! keeps the questions, which are the part worth getting right, testable without a world.

use ferrumc_entities::entity_type::{EntityType, MobCategory, SpawnPlacement};
use rand::Rng;

mod caps;
mod rules;

pub use caps::SpawnState;
pub use rules::rule_holds;

/// A block in the world.
pub type Pos = bevy_math::IVec3;

/// How close a player has to be for a mob never to appear.
///
/// Vanilla compares squared distances, so this is the square of twenty-four blocks.
const TOO_CLOSE_TO_A_PLAYER: f64 = 576.0;

/// How many separate attempts a chunk gets each time it is tried.
const GROUPS_PER_CHUNK: usize = 3;

/// How far the members of one group wander from the first of them, per step.
const GROUP_SPREAD: i32 = 6;

/// The largest group a single attempt may put down, before a count from the biome replaces it.
const FIRST_GUESS_AT_A_GROUP: u32 = 4;

/// What a biome says lives in it, for one category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spawner {
    pub kind: EntityType,
    pub weight: u32,
    pub min_count: u32,
    pub max_count: u32,
}

/// One mob to put down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spawned {
    pub kind: EntityType,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
}

/// Everything the questions need to ask of the world.
pub trait SpawnWorld {
    fn block_at(&self, pos: Pos) -> ferrumc_world::block_state_id::BlockStateId;

    /// How much light a block gives off reaches here.
    fn block_light(&self, pos: Pos) -> u8;

    /// How much daylight reaches here, before the time of day is taken off it.
    fn sky_light(&self, pos: Pos) -> u8;

    /// Whether there is nothing but air between here and the sky.
    fn can_see_sky(&self, pos: Pos) -> bool;

    /// How dark it is here all told, which is the greater of what the sky and the blocks give.
    fn brightness(&self, pos: Pos) -> u8;

    /// What lives in the biome here, of this category.
    fn spawners_at(&self, pos: Pos, category: MobCategory) -> &[Spawner];

    /// How far above the highest thing at this column a mob would stand.
    fn surface_at(&self, x: i32, z: i32) -> i32;

    /// The lowest block the world has.
    fn min_y(&self) -> i32;

    /// How far the nearest player is, squared, or nothing where there is none.
    fn nearest_player_sqr(&self, x: f64, y: f64, z: f64) -> Option<f64>;

    /// Whether a mob of this kind fits here without being inside anything.
    fn fits(&self, kind: EntityType, x: f64, y: f64, z: f64) -> bool;

    /// Whether a block is one of those a mob may stand on, per the block's own answer.
    fn standable(&self, pos: Pos) -> bool;

    /// Whether the block here is water.
    fn is_water(&self, pos: Pos) -> bool;

    /// Whether the block here stops a mob from being put in it.
    fn is_solid(&self, pos: Pos) -> bool;
}

/// Tries to put a group of mobs of one category somewhere in a chunk.
///
/// Three attempts, each starting from the same random position and wandering from it, each drawing
/// one kind from the biome and putting down as many of it as the biome allows. Vanilla gives up on
/// a whole attempt the moment one question fails, which is why so few mobs appear.
pub fn spawn_in_chunk(
    world: &dyn SpawnWorld,
    state: &mut SpawnState,
    category: MobCategory,
    chunk_x: i32,
    chunk_z: i32,
    rng: &mut impl Rng,
) -> Vec<Spawned> {
    let start = random_position(world, chunk_x, chunk_z, rng);
    if start.y < world.min_y() + 1 || world.is_solid(start) {
        return Vec::new();
    }

    let mut put_down = Vec::new();
    for _ in 0..GROUPS_PER_CHUNK {
        let mut x = start.x;
        let mut z = start.z;
        let mut drawn: Option<Spawner> = None;
        // How many to try for. This starts as a guess and is replaced by the biome's own count
        // the moment a kind is drawn, and the loop has to see the new number rather than the guess
        // — vanilla reads it afresh every time round, which is what makes a herd a herd.
        let mut wanted = rng.gen_range(1..=FIRST_GUESS_AT_A_GROUP);
        let mut in_group = 0;
        let mut tried = 0;

        while tried < wanted {
            tried += 1;
            x += rng.gen_range(0..GROUP_SPREAD) - rng.gen_range(0..GROUP_SPREAD);
            z += rng.gen_range(0..GROUP_SPREAD) - rng.gen_range(0..GROUP_SPREAD);
            let at = Pos::new(x, start.y, z);
            let (centre_x, centre_z) = (f64::from(x) + 0.5, f64::from(z) + 0.5);

            let Some(player_sqr) = world.nearest_player_sqr(centre_x, f64::from(start.y), centre_z)
            else {
                continue;
            };
            if player_sqr <= TOO_CLOSE_TO_A_PLAYER {
                continue;
            }

            let spawner = match drawn {
                Some(spawner) => spawner,
                None => {
                    let Some(spawner) = draw(world.spawners_at(at, category), rng) else {
                        break;
                    };
                    drawn = Some(spawner);
                    wanted = spawner.min_count
                        + rng.gen_range(0..=(spawner.max_count.saturating_sub(spawner.min_count)));
                    spawner
                }
            };

            if !may_appear(world, spawner.kind, at, player_sqr, rng) {
                continue;
            }
            if !state.may_add(spawner.kind.category(), chunk_x, chunk_z) {
                return put_down;
            }

            state.added(spawner.kind.category(), chunk_x, chunk_z);
            put_down.push(Spawned {
                kind: spawner.kind,
                x: centre_x,
                y: f64::from(start.y),
                z: centre_z,
                yaw: rng.gen_range(0.0..360.0),
            });
            in_group += 1;
            if in_group >= wanted {
                break;
            }
        }
    }
    put_down
}

/// Whether a mob of this kind may appear here.
///
/// The order is vanilla's: the cheap questions about the type first, then the ones that read the
/// world, then the one that has to build a box and look for what is in the way.
fn may_appear(
    world: &dyn SpawnWorld,
    kind: EntityType,
    at: Pos,
    player_sqr: f64,
    rng: &mut impl Rng,
) -> bool {
    let def = kind.def();
    if def.category == MobCategory::Misc || !def.summon {
        return false;
    }
    // A kind that will not spawn far from a player is not put beyond the distance at which it
    // would despawn again on the next tick.
    let despawn = f64::from(def.category.def().despawn_distance);
    if !def.spawn_far_from_player && player_sqr > despawn * despawn {
        return false;
    }
    if !placement_holds(world, kind, at) {
        return false;
    }
    if !rule_holds(world, kind, at, rng) {
        return false;
    }
    world.fits(
        kind,
        f64::from(at.x) + 0.5,
        f64::from(at.y),
        f64::from(at.z) + 0.5,
    )
}

/// Whether the ground here is the kind this mob stands on.
fn placement_holds(world: &dyn SpawnWorld, kind: EntityType, at: Pos) -> bool {
    let below = at - Pos::Y;
    match kind.def().placement {
        SpawnPlacement::OnGround => world.standable(below) && !world.is_solid(at),
        SpawnPlacement::InWater => world.is_water(at) && !world.is_solid(at - Pos::Y),
        SpawnPlacement::InLava => !world.is_solid(at),
        SpawnPlacement::NoRestrictions => true,
    }
}

/// Picks one kind out of what a biome offers, by weight.
fn draw(spawners: &[Spawner], rng: &mut impl Rng) -> Option<Spawner> {
    let total: u32 = spawners.iter().map(|s| s.weight).sum();
    if total == 0 {
        return None;
    }
    let mut roll = rng.gen_range(0..total);
    for spawner in spawners {
        if roll < spawner.weight {
            return Some(*spawner);
        }
        roll -= spawner.weight;
    }
    None
}

/// A position anywhere in a chunk's column, from the floor to just above whatever is highest.
fn random_position(world: &dyn SpawnWorld, chunk_x: i32, chunk_z: i32, rng: &mut impl Rng) -> Pos {
    let x = chunk_x * 16 + rng.gen_range(0..16);
    let z = chunk_z * 16 + rng.gen_range(0..16);
    let top = world.surface_at(x, z) + 1;
    let y = rng.gen_range(world.min_y()..=top.max(world.min_y()));
    Pos::new(x, y, z)
}

/// How far a mob wanders looking for somewhere to stand when a chunk is first made.
const SETTLING_SPREAD: i32 = 5;

/// How many places one mob tries before it is given up on.
const TRIES_PER_MOB: usize = 4;

/// How many herds one chunk may be given.
///
/// Vanilla keeps rolling against the biome's probability with nothing to stop it, which terminates
/// only because every biome's number is small. A pack that said one would hang the server, so the
/// rolling is bounded here.
const HERDS_PER_CHUNK: usize = 16;

/// Puts a herd in a chunk that has just been made.
///
/// This is why a fresh world already has animals in it: the chunk is populated as it is generated,
/// rather than waiting for the spawn loop to find it. Only the group that grows up is placed this
/// way, and how likely it is comes from the biome.
pub fn populate_new_chunk(
    world: &dyn SpawnWorld,
    chunk_x: i32,
    chunk_z: i32,
    probability: f32,
    rng: &mut impl Rng,
) -> Vec<Spawned> {
    let (min_x, min_z) = (chunk_x * 16, chunk_z * 16);
    let spawners = world.spawners_at(Pos::new(min_x, 0, min_z), MobCategory::Creature);
    if spawners.is_empty() {
        return Vec::new();
    }

    let mut put_down = Vec::new();
    for _ in 0..HERDS_PER_CHUNK {
        if rng.gen_range(0.0..1.0) >= probability {
            break;
        }
        let Some(spawner) = draw(spawners, rng) else {
            break;
        };
        let wanted = spawner.min_count
            + rng.gen_range(0..=(spawner.max_count.saturating_sub(spawner.min_count)));
        let (start_x, start_z) = (min_x + rng.gen_range(0..16), min_z + rng.gen_range(0..16));
        let (mut x, mut z) = (start_x, start_z);

        for _ in 0..wanted {
            for _ in 0..TRIES_PER_MOB {
                let y = world.surface_at(x, z);
                let at = Pos::new(x, y, z);
                if spawner.kind.def().summon
                    && placement_holds(world, spawner.kind, at)
                    && rule_holds(world, spawner.kind, at, rng)
                {
                    let (centre_x, centre_z) = (f64::from(x) + 0.5, f64::from(z) + 0.5);
                    if world.fits(spawner.kind, centre_x, f64::from(y), centre_z) {
                        put_down.push(Spawned {
                            kind: spawner.kind,
                            x: centre_x,
                            y: f64::from(y),
                            z: centre_z,
                            yaw: rng.gen_range(0.0..360.0),
                        });
                        break;
                    }
                }
                // Wander, and come back inside the chunk if the wandering left it.
                x += rng.gen_range(0..SETTLING_SPREAD) - rng.gen_range(0..SETTLING_SPREAD);
                z += rng.gen_range(0..SETTLING_SPREAD) - rng.gen_range(0..SETTLING_SPREAD);
                if x < min_x || x >= min_x + 16 || z < min_z || z >= min_z + 16 {
                    x = start_x;
                    z = start_z;
                }
            }
        }
    }
    put_down
}

/// Whether a mob that is this far from the nearest player should be taken away again.
///
/// Vanilla removes one past its category's despawn distance at once, and gives one between that
/// and the distance it is safe at a small chance each tick. A mob that was asked to stay, or whose
/// category always stays, is left alone.
#[must_use]
pub fn should_despawn(
    kind: EntityType,
    player_sqr: Option<f64>,
    persistent: bool,
    rng: &mut impl Rng,
) -> bool {
    let category = kind.category().def();
    if persistent || category.persistent {
        return false;
    }
    let Some(player_sqr) = player_sqr else {
        // Nobody is watching at all, which is the one case vanilla removes without a roll.
        return true;
    };
    let despawn = f64::from(category.despawn_distance);
    if player_sqr > despawn * despawn {
        return true;
    }
    let safe = f64::from(category.no_despawn_distance);
    player_sqr > safe * safe && rng.gen_range(0..800) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrumc_world::block_state_id::BlockStateId;
    use rand::SeedableRng;

    /// A meadow: flat ground at sea level, bright, with cows in it.
    struct Meadow {
        spawners: Vec<Spawner>,
        player_at: Option<f64>,
    }

    impl SpawnWorld for Meadow {
        fn block_at(&self, _pos: Pos) -> BlockStateId {
            BlockStateId::default()
        }
        fn block_light(&self, _pos: Pos) -> u8 {
            0
        }
        fn sky_light(&self, _pos: Pos) -> u8 {
            15
        }
        fn can_see_sky(&self, pos: Pos) -> bool {
            pos.y >= 64
        }
        fn brightness(&self, _pos: Pos) -> u8 {
            15
        }
        fn spawners_at(&self, _pos: Pos, category: MobCategory) -> &[Spawner] {
            if category == MobCategory::Creature {
                &self.spawners
            } else {
                &[]
            }
        }
        fn surface_at(&self, _x: i32, _z: i32) -> i32 {
            64
        }
        fn min_y(&self) -> i32 {
            -64
        }
        fn nearest_player_sqr(&self, _x: f64, _y: f64, _z: f64) -> Option<f64> {
            self.player_at
        }
        fn fits(&self, _k: EntityType, _x: f64, _y: f64, _z: f64) -> bool {
            true
        }
        fn standable(&self, pos: Pos) -> bool {
            pos.y < 64
        }
        fn is_water(&self, _pos: Pos) -> bool {
            false
        }
        fn is_solid(&self, pos: Pos) -> bool {
            pos.y < 64
        }
    }

    fn meadow(player_at: Option<f64>) -> Meadow {
        Meadow {
            spawners: vec![Spawner {
                kind: EntityType::Cow,
                weight: 1,
                min_count: 2,
                max_count: 4,
            }],
            player_at,
        }
    }

    fn rng(seed: u64) -> rand::rngs::StdRng {
        rand::rngs::StdRng::seed_from_u64(seed)
    }

    #[test]
    fn a_new_chunk_gets_a_herd() {
        // Always, so the loop runs at least once rather than depending on the roll.
        // Certain, so the test does not depend on a roll. Vanilla's own numbers are around a
        // tenth, and a biome that said one would once have gone round for ever.
        let put = populate_new_chunk(&meadow(None), 0, 0, 1.0, &mut rng(7));
        assert!(!put.is_empty(), "a meadow should end up with cows in it");
        assert!(put.iter().all(|cow| cow.kind == EntityType::Cow));
        assert!(
            put.iter().all(|cow| (0.0..16.0).contains(&cow.x)),
            "and they should be in the chunk that was made, not beside it"
        );
    }

    #[test]
    fn a_biome_with_nothing_in_it_gets_nothing() {
        let barren = Meadow {
            spawners: Vec::new(),
            player_at: None,
        };
        assert!(populate_new_chunk(&barren, 0, 0, 1.0, &mut rng(7)).is_empty());
    }

    #[test]
    fn nothing_appears_next_to_a_player() {
        let mut counts = SpawnState::new(100_000);
        let close = meadow(Some(4.0));
        let put = spawn_in_chunk(
            &close,
            &mut counts,
            MobCategory::Creature,
            0,
            0,
            &mut rng(3),
        );
        assert!(
            put.is_empty(),
            "twenty-four blocks is the closest anything gets"
        );
    }

    #[test]
    fn nothing_appears_where_nobody_is() {
        // Vanilla asks how far the nearest player is and gives up when there is none, which is
        // what keeps a server with nobody on it from filling with mobs.
        let mut counts = SpawnState::new(100_000);
        let empty = meadow(None);
        let put = spawn_in_chunk(
            &empty,
            &mut counts,
            MobCategory::Creature,
            0,
            0,
            &mut rng(3),
        );
        assert!(put.is_empty());
    }

    #[test]
    fn a_full_world_stops_putting_more_down() {
        let far = meadow(Some(10_000.0));
        let mut counts = SpawnState::new(1);
        let put = spawn_in_chunk(&far, &mut counts, MobCategory::Creature, 0, 0, &mut rng(3));
        assert!(put.is_empty(), "one chunk of world has room for nothing");
    }

    #[test]
    fn a_mob_far_from_everyone_is_taken_away() {
        let far = f64::from(MobCategory::Monster.def().despawn_distance) + 1.0;
        assert!(should_despawn(
            EntityType::Zombie,
            Some(far * far),
            false,
            &mut rng(1)
        ));
    }

    #[test]
    fn a_mob_beside_someone_stays() {
        assert!(!should_despawn(
            EntityType::Zombie,
            Some(4.0),
            false,
            &mut rng(1)
        ));
    }

    #[test]
    fn a_mob_that_was_asked_to_stay_stays_wherever_it_is() {
        assert!(!should_despawn(EntityType::Zombie, None, true, &mut rng(1)));
    }
}
