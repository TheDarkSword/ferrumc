//! How much attention each chunk gets.
//!
//! A chunk is not simply loaded or not. Vanilla gives every chunk a *level*, a number that says how
//! close it is to something that cares about it, and what the server does with a chunk follows from
//! that number: the ones nearest a player have their entities ticked, a ring further out has its
//! blocks ticked, a ring further still is loaded and sent but does nothing, and beyond that a chunk
//! is not kept at all.
//!
//! Levels come from *tickets*. Something that wants a chunk kept asks for it at a level, and the
//! level spreads outwards a step at a time, so one ticket keeps a whole neighbourhood at
//! progressively less attention. A player carries two: one for what they can see, and a tighter one
//! for what goes on around them.
//!
//! Follows `ChunkLevel` and `DistanceManager` in the vanilla sources; the numbers are vanilla's, so
//! a distance means the same here as there.

use crate::pos::ChunkPos;
use std::collections::HashMap;
use std::collections::VecDeque;

/// Entities in a chunk at this level or below are ticked.
pub const ENTITY_TICKING: u32 = 31;
/// Blocks in a chunk at this level or below are ticked.
pub const BLOCK_TICKING: u32 = 32;
/// A chunk at this level or below is loaded, and can be sent to a player.
pub const FULL: u32 = 33;
/// Past this, a chunk is not kept.
pub const MAX_LEVEL: u32 = FULL + 1;

/// What a level means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChunkStatus {
    /// Not kept.
    Inaccessible,
    /// Kept and sendable, but nothing in it happens.
    Full,
    /// Its blocks tick: crops grow, fluids move, scheduled ticks run.
    BlockTicking,
    /// Its entities tick as well.
    EntityTicking,
}

impl ChunkStatus {
    /// What a level amounts to.
    #[must_use]
    pub const fn of(level: u32) -> Self {
        if level <= ENTITY_TICKING {
            Self::EntityTicking
        } else if level <= BLOCK_TICKING {
            Self::BlockTicking
        } else if level <= FULL {
            Self::Full
        } else {
            Self::Inaccessible
        }
    }

    /// Whether a chunk at this status is kept at all.
    #[must_use]
    pub const fn is_loaded(self) -> bool {
        !matches!(self, Self::Inaccessible)
    }

    /// Whether its blocks take their turns.
    #[must_use]
    pub const fn ticks_blocks(self) -> bool {
        matches!(self, Self::BlockTicking | Self::EntityTicking)
    }

    /// Whether its entities take theirs.
    #[must_use]
    pub const fn ticks_entities(self) -> bool {
        matches!(self, Self::EntityTicking)
    }
}

/// One set of tickets and the levels they spread to.
#[derive(Debug, Default)]
struct Tracker {
    tickets: HashMap<ChunkPos, u32>,
    levels: HashMap<ChunkPos, u32>,
}

impl Tracker {
    fn clear(&mut self) {
        self.tickets.clear();
    }

    fn add(&mut self, pos: ChunkPos, level: u32) {
        let entry = self.tickets.entry(pos).or_insert(MAX_LEVEL);
        *entry = (*entry).min(level);
    }

    /// One step of distance costs one level.
    fn recompute(&mut self) {
        self.levels.clear();
        let mut queue: VecDeque<(ChunkPos, u32)> = VecDeque::new();
        for (&pos, &level) in &self.tickets {
            if level <= MAX_LEVEL {
                self.levels.insert(pos, level);
                queue.push_back((pos, level));
            }
        }

        while let Some((pos, level)) = queue.pop_front() {
            let next = level + 1;
            if next > MAX_LEVEL {
                continue;
            }
            for neighbour in [
                ChunkPos::new(pos.x() - 1, pos.z()),
                ChunkPos::new(pos.x() + 1, pos.z()),
                ChunkPos::new(pos.x(), pos.z() - 1),
                ChunkPos::new(pos.x(), pos.z() + 1),
            ] {
                let current = self
                    .levels
                    .get(&neighbour)
                    .copied()
                    .unwrap_or(MAX_LEVEL + 1);
                if next < current {
                    self.levels.insert(neighbour, next);
                    queue.push_back((neighbour, next));
                }
            }
        }
    }

    fn level(&self, pos: ChunkPos) -> u32 {
        self.levels.get(&pos).copied().unwrap_or(MAX_LEVEL + 1)
    }
}

/// Which chunks are kept, and how closely.
///
/// Two separate questions, so two separate sets of levels, as vanilla has: what is kept and
/// sendable is not what ticks. One set would make a chunk near a player tick because the player can
/// see far, which is not what a simulation distance is for.
#[derive(Debug, Default)]
pub struct ChunkLevels {
    loading: Tracker,
    simulation: Tracker,
}

impl ChunkLevels {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets every ticket. Called before the tickets for a tick are put back.
    pub fn clear(&mut self) {
        self.loading.clear();
        self.simulation.clear();
    }

    /// Asks for a chunk to be kept, without asking for anything in it to happen.
    pub fn keep(&mut self, pos: ChunkPos, level: u32) {
        self.loading.add(pos, level);
    }

    /// A player asks for two things: everything they can see kept and sendable, and everything
    /// close to them ticking.
    ///
    /// Both tickets are placed so that the last chunk at the given distance is exactly at its
    /// threshold, which is how a distance in the config means the same as a distance in the game.
    pub fn add_player(&mut self, pos: ChunkPos, view_distance: u32, simulation_distance: u32) {
        self.loading.add(pos, FULL.saturating_sub(view_distance));
        self.simulation
            .add(pos, ENTITY_TICKING.saturating_sub(simulation_distance));
    }

    /// Works out what every chunk ends up at.
    pub fn recompute(&mut self) {
        self.loading.recompute();
        self.simulation.recompute();
    }

    /// What this chunk amounts to: kept, ticking blocks, ticking entities, or nothing.
    #[must_use]
    pub fn status(&self, pos: ChunkPos) -> ChunkStatus {
        if ChunkStatus::of(self.loading.level(pos)) == ChunkStatus::Inaccessible {
            return ChunkStatus::Inaccessible;
        }
        // It is kept. Whether anything in it happens is the other question.
        match ChunkStatus::of(self.simulation.level(pos)) {
            ChunkStatus::Inaccessible => ChunkStatus::Full,
            ticking => ticking,
        }
    }

    /// How far this chunk is from being let go of, for anything that wants the number.
    #[must_use]
    pub fn level(&self, pos: ChunkPos) -> u32 {
        self.loading.level(pos)
    }

    /// Every chunk that is kept, with what it is kept at.
    pub fn loaded(&self) -> impl Iterator<Item = (ChunkPos, ChunkStatus)> + '_ {
        self.loading
            .levels
            .keys()
            .map(|&pos| (pos, self.status(pos)))
            .filter(|(_, status)| status.is_loaded())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: i32, z: i32) -> ChunkPos {
        ChunkPos::new(x, z)
    }

    /// The point of levels: near a player a chunk ticks, further out it is only kept and sent, and
    /// past that it is not kept at all. One player, three rings.
    #[test]
    fn a_chunk_at_the_edge_of_sight_is_loaded_but_not_ticking() {
        let mut levels = ChunkLevels::new();
        levels.add_player(at(0, 0), 12, 4);
        levels.recompute();

        assert_eq!(levels.status(at(0, 0)), ChunkStatus::EntityTicking);
        assert_eq!(
            levels.status(at(4, 0)),
            ChunkStatus::EntityTicking,
            "the last chunk within the simulation distance still ticks"
        );
        assert_eq!(
            levels.status(at(5, 0)),
            ChunkStatus::BlockTicking,
            "one past it the entities stop and the blocks carry on, as they do in the game"
        );
        assert_eq!(
            levels.status(at(6, 0)),
            ChunkStatus::Full,
            "one past that nothing in it happens"
        );
        assert!(
            levels.status(at(12, 0)).is_loaded(),
            "the last chunk within sight is still kept and sendable"
        );
        assert!(
            !levels.status(at(13, 0)).is_loaded(),
            "one past sight is not kept"
        );
    }

    /// Distance is counted in steps, so a chunk on the diagonal is as far as the two steps it takes
    /// to reach it.
    #[test]
    fn distance_is_counted_in_steps() {
        let mut levels = ChunkLevels::new();
        levels.add_player(at(0, 0), 8, 2);
        levels.recompute();

        assert_eq!(levels.level(at(1, 1)), levels.level(at(2, 0)));
        assert!(levels.level(at(1, 1)) > levels.level(at(1, 0)));
    }

    /// Two players' chunks each take the best of what either asks for, rather than the nearer one
    /// losing to the further.
    #[test]
    fn the_closest_player_decides() {
        let mut levels = ChunkLevels::new();
        levels.add_player(at(0, 0), 8, 2);
        levels.add_player(at(20, 0), 8, 2);
        levels.recompute();

        assert_eq!(levels.status(at(20, 0)), ChunkStatus::EntityTicking);
        // Between them, kept by whichever is nearer.
        assert!(levels.status(at(8, 0)).is_loaded());
        assert!(levels.status(at(12, 0)).is_loaded());
        // In the gap neither reaches, nothing is kept.
        assert!(!levels.status(at(10, 0)).is_loaded());
    }

    /// Tickets are put back every tick, so a player walking away lets go of what they were holding.
    #[test]
    fn clearing_the_tickets_lets_go() {
        let mut levels = ChunkLevels::new();
        levels.add_player(at(0, 0), 8, 2);
        levels.recompute();
        assert!(levels.status(at(0, 0)).is_loaded());

        levels.clear();
        levels.recompute();
        assert!(!levels.status(at(0, 0)).is_loaded());
        assert_eq!(levels.loaded().count(), 0);
    }
}
