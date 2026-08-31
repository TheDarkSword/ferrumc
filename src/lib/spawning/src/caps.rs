//! How many mobs of a kind the world will hold.
//!
//! Two limits, and a mob has to be under both. One counts every mob of a category in the world
//! against the number of chunks players are keeping loaded, which is what stops a large server
//! filling up. The other counts them per chunk near a player, which is what stops them all
//! appearing in one place.

use ferrumc_entities::entity_type::MobCategory;
use std::collections::HashMap;

/// The number of chunks vanilla treats as one player's worth when working out the world limit.
///
/// It is the area of the seventeen-by-seventeen square around a player, and it is written into the
/// game as a magic number rather than derived from anything.
const CHUNKS_PER_PLAYER: u32 = 17 * 17;

/// How many chunks around a player are counted for the per-chunk limit.
const NEAR_A_PLAYER: i32 = 8;

/// How many of each category are about, and where.
#[derive(Debug, Default, Clone)]
pub struct SpawnState {
    /// How many chunks are loaded and could hold a mob.
    spawnable_chunks: u32,
    /// How many of each category there are in the world.
    world_wide: HashMap<MobCategory, u32>,
    /// How many of each category are in each chunk near a player.
    per_chunk: HashMap<(i32, i32), HashMap<MobCategory, u32>>,
}

impl SpawnState {
    /// A count of what is already about, before a tick's attempts are made.
    #[must_use]
    pub fn new(spawnable_chunks: u32) -> Self {
        Self {
            spawnable_chunks,
            ..Self::default()
        }
    }

    /// Counts a mob that already exists.
    pub fn count(&mut self, category: MobCategory, chunk_x: i32, chunk_z: i32) {
        self.added(category, chunk_x, chunk_z);
    }

    /// Whether the world as a whole has room for another of this category.
    ///
    /// The limit grows with how much of the world is loaded, so one player alone sees far fewer
    /// mobs than a crowded server does.
    #[must_use]
    pub fn has_room_in_the_world(&self, category: MobCategory) -> bool {
        let Some(per_chunk) = category.def().max_per_chunk else {
            return true;
        };
        let limit = per_chunk * self.spawnable_chunks / CHUNKS_PER_PLAYER;
        self.world_wide.get(&category).copied().unwrap_or(0) < limit
    }

    /// Whether the chunks around this one have room for another of this category.
    #[must_use]
    pub fn has_room_here(&self, category: MobCategory, chunk_x: i32, chunk_z: i32) -> bool {
        let Some(limit) = category.def().max_per_chunk else {
            return true;
        };
        let mut near = 0;
        for x in chunk_x - NEAR_A_PLAYER..=chunk_x + NEAR_A_PLAYER {
            for z in chunk_z - NEAR_A_PLAYER..=chunk_z + NEAR_A_PLAYER {
                near += self
                    .per_chunk
                    .get(&(x, z))
                    .and_then(|counts| counts.get(&category))
                    .copied()
                    .unwrap_or(0);
            }
        }
        near < limit
    }

    /// Both limits at once, which is what an attempt asks.
    #[must_use]
    pub fn may_add(&self, category: MobCategory, chunk_x: i32, chunk_z: i32) -> bool {
        self.has_room_in_the_world(category) && self.has_room_here(category, chunk_x, chunk_z)
    }

    /// Records that one was put down, so the rest of the tick sees it.
    pub fn added(&mut self, category: MobCategory, chunk_x: i32, chunk_z: i32) {
        *self.world_wide.entry(category).or_insert(0) += 1;
        *self
            .per_chunk
            .entry((chunk_x, chunk_z))
            .or_default()
            .entry(category)
            .or_insert(0) += 1;
    }

    /// How many of a category are about.
    #[must_use]
    pub fn how_many(&self, category: MobCategory) -> u32 {
        self.world_wide.get(&category).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_players_worth_of_chunks_is_one_players_worth_of_mobs() {
        // The limit is the category's own number scaled by how much of the world is loaded, so a
        // world with exactly one player's worth of chunks allows exactly that number.
        let per_chunk = MobCategory::Monster
            .def()
            .max_per_chunk
            .expect("monsters have a limit");
        let mut state = SpawnState::new(CHUNKS_PER_PLAYER);

        for _ in 0..per_chunk {
            assert!(state.has_room_in_the_world(MobCategory::Monster));
            state.added(MobCategory::Monster, 0, 0);
        }
        assert!(!state.has_room_in_the_world(MobCategory::Monster));
    }

    #[test]
    fn a_barely_loaded_world_holds_barely_any() {
        let state = SpawnState::new(1);
        assert!(
            !state.has_room_in_the_world(MobCategory::Monster),
            "one chunk is not enough of the world to be worth a monster"
        );
    }

    #[test]
    fn a_crowd_in_one_place_stops_more_appearing_beside_it() {
        let limit = MobCategory::Monster
            .def()
            .max_per_chunk
            .expect("monsters have a limit");
        let mut state = SpawnState::new(CHUNKS_PER_PLAYER * 1000);

        for _ in 0..limit {
            state.added(MobCategory::Monster, 0, 0);
        }
        assert!(
            state.has_room_in_the_world(MobCategory::Monster),
            "the world at large is nowhere near full"
        );
        assert!(!state.has_room_here(MobCategory::Monster, 0, 0));
        assert!(
            !state.has_room_here(MobCategory::Monster, NEAR_A_PLAYER, 0),
            "a chunk within sight of the crowd is just as full"
        );
        assert!(
            state.has_room_here(MobCategory::Monster, NEAR_A_PLAYER + 1, 0),
            "one beyond it is not"
        );
    }

    #[test]
    fn the_group_that_is_not_a_mob_is_never_full() {
        // Everything the game does not count as a mob is in one group with no limit at all, and
        // the spawn loop never asks about it — but nothing here should refuse it either.
        assert!(MobCategory::Misc.def().max_per_chunk.is_none());
        let mut state = SpawnState::new(1);
        for _ in 0..10_000 {
            state.added(MobCategory::Misc, 0, 0);
        }
        assert!(state.may_add(MobCategory::Misc, 0, 0));
    }
}
