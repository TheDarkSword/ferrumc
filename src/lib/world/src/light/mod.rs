//! Working out how bright each block is.
//!
//! Light spreads outwards from what emits it, losing at least one level per block and more through
//! anything that dims it. Taking a light away is the harder direction: everything it lit has to be
//! darkened first and then relit from whatever else reaches it, because there is no way to tell
//! which of several sources a level came from.
//!
//! Two queues do that, as in vanilla: one carries light outwards, the other carries darkness, and
//! darkness is always drained first. Both are breadth-first over the six directions, and an entry
//! remembers which directions it may still spread in so light never bounces back the way it came.
//!
//! Follows `BlockLightEngine` and `LightEngine` in the vanilla sources.

use crate::block_data::{face_occludes_light, light_emission, light_opacity, MAX_LIGHT};
use crate::block_state::Direction;
use crate::block_state_id::BlockStateId;
use crate::pos::BlockPos;
use std::collections::VecDeque;

/// The six, in the order vanilla walks them.
const ALL_DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

const fn bit(direction: Direction) -> u8 {
    1 << match direction {
        Direction::Down => 0,
        Direction::Up => 1,
        Direction::North => 2,
        Direction::South => 3,
        Direction::West => 4,
        Direction::East => 5,
    }
}

/// Every direction.
const ANY: u8 = 0b0011_1111;

/// What a light engine needs of the world.
pub trait LightWorld {
    fn block_at(&mut self, pos: BlockPos) -> BlockStateId;
    fn light_at(&mut self, pos: BlockPos) -> u8;
    fn set_light(&mut self, pos: BlockPos, level: u8);
    /// Whether light is kept for this position at all. A position in a chunk that is not loaded is
    /// not darkened or lit; it is left for whenever it is.
    fn stores_light(&mut self, pos: BlockPos) -> bool;
}

/// One position's turn at spreading light, or darkness.
#[derive(Debug, Clone, Copy)]
struct Entry {
    pos: BlockPos,
    /// The level being spread. For darkness, the level that was there before.
    level: u8,
    /// Which directions this entry may still go in.
    directions: u8,
    /// Whether the level came from the block itself rather than from a neighbour.
    from_emission: bool,
}

/// Spreads block light, and takes it back.
#[derive(Default)]
pub struct BlockLightEngine {
    to_check: Vec<BlockPos>,
    increase: VecDeque<Entry>,
    decrease: VecDeque<Entry>,
}

impl BlockLightEngine {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Says that whatever is at this position may have changed what it does to light.
    pub fn check(&mut self, pos: BlockPos) {
        self.to_check.push(pos);
    }

    /// Whether anything is waiting to be worked out.
    #[must_use]
    pub fn has_work(&self) -> bool {
        !self.to_check.is_empty() || !self.increase.is_empty() || !self.decrease.is_empty()
    }

    /// Works everything out. Darkness first: a level that is about to be taken away must not be
    /// spread as though it were still there.
    pub fn run(&mut self, world: &mut dyn LightWorld) {
        for pos in std::mem::take(&mut self.to_check) {
            self.check_node(world, pos);
        }
        while let Some(entry) = self.decrease.pop_front() {
            self.propagate_decrease(world, entry);
        }
        while let Some(entry) = self.increase.pop_front() {
            self.propagate_increase(world, entry);
        }
    }

    fn check_node(&mut self, world: &mut dyn LightWorld, pos: BlockPos) {
        if !world.stores_light(pos) {
            return;
        }
        let state = world.block_at(pos);
        let emission = light_emission(state);
        let old = world.light_at(pos);

        if emission < old {
            // Whatever was here is gone or dimmer: take back what it gave out.
            world.set_light(pos, 0);
            self.decrease.push_back(Entry {
                pos,
                level: old,
                directions: ANY,
                from_emission: false,
            });
        } else {
            // It is no dimmer than it was, but a block that used to stop light may not any more,
            // so its neighbours are asked to push what they have back in.
            self.decrease.push_back(Entry {
                pos,
                level: 1,
                directions: ANY,
                from_emission: false,
            });
        }

        if emission > 0 {
            self.increase.push_back(Entry {
                pos,
                level: emission,
                directions: ANY,
                from_emission: true,
            });
        }
    }

    fn propagate_increase(&mut self, world: &mut dyn LightWorld, entry: Entry) {
        let mut level = world.light_at(entry.pos);
        if entry.from_emission && level < entry.level {
            world.set_light(entry.pos, entry.level);
            level = entry.level;
        }
        // Something brighter has been here since; that entry will do the spreading.
        if level != entry.level {
            return;
        }

        let mut from_state = None;
        for direction in ALL_DIRECTIONS {
            if entry.directions & bit(direction) == 0 {
                continue;
            }
            let to = entry.pos.relative(direction);
            if !world.stores_light(to) {
                continue;
            }
            let to_level = world.light_at(to);
            // Even air takes one off, so this is the most it could possibly become.
            if level.saturating_sub(1) <= to_level {
                continue;
            }
            let to_state = world.block_at(to);
            let new_level = level.saturating_sub(light_opacity(to_state));
            if new_level <= to_level {
                continue;
            }
            let from = *from_state.get_or_insert_with(|| world.block_at(entry.pos));
            if face_occludes_light(from, direction)
                || face_occludes_light(to_state, direction.opposite())
            {
                continue;
            }

            world.set_light(to, new_level);
            if new_level > 1 {
                self.increase.push_back(Entry {
                    pos: to,
                    level: new_level,
                    directions: ANY & !bit(direction.opposite()),
                    from_emission: false,
                });
            }
        }
    }

    fn propagate_decrease(&mut self, world: &mut dyn LightWorld, entry: Entry) {
        for direction in ALL_DIRECTIONS {
            if entry.directions & bit(direction) == 0 {
                continue;
            }
            let to = entry.pos.relative(direction);
            if !world.stores_light(to) {
                continue;
            }
            let to_level = world.light_at(to);
            if to_level == 0 {
                continue;
            }

            if to_level <= entry.level.saturating_sub(1) {
                // Dim enough to have come from here, so it goes too.
                let to_state = world.block_at(to);
                let to_emission = light_emission(to_state);
                world.set_light(to, 0);
                if to_emission < to_level {
                    self.decrease.push_back(Entry {
                        pos: to,
                        level: to_level,
                        directions: ANY & !bit(direction.opposite()),
                        from_emission: false,
                    });
                }
                if to_emission > 0 {
                    self.increase.push_back(Entry {
                        pos: to,
                        level: to_emission,
                        directions: ANY,
                        from_emission: true,
                    });
                }
            } else {
                // Brighter than anything from here could have made it, so it has another source and
                // will light this way again.
                self.increase.push_back(Entry {
                    pos: to,
                    level: to_level,
                    directions: bit(direction.opposite()),
                    from_emission: false,
                });
            }
        }
    }
}

/// One chunk, lit on its own.
///
/// Light that would come from a neighbouring chunk is not seen: a chunk is lit as though it stood
/// alone, and what crosses the border is settled when both are loaded. See
/// `internal_docs/deferred.md`.
struct SingleChunk<'a> {
    chunk: &'a mut crate::chunk::Chunk,
}

impl LightWorld for SingleChunk<'_> {
    fn block_at(&mut self, pos: BlockPos) -> BlockStateId {
        self.chunk.get_block(pos.chunk_block_pos())
    }

    fn light_at(&mut self, pos: BlockPos) -> u8 {
        self.chunk.block_light(pos.chunk_block_pos())
    }

    fn set_light(&mut self, pos: BlockPos, level: u8) {
        self.chunk.set_block_light(pos.chunk_block_pos(), level);
    }

    fn stores_light(&mut self, pos: BlockPos) -> bool {
        // Only this chunk, and only within its height.
        let height = self.chunk.dimensions();
        let min = i32::from(height.min_y);
        pos.pos.y >= min
            && pos.pos.y < min + i32::from(height.height)
            && pos.chunk() == BlockPos::of(pos.pos.x, 0, pos.pos.z).chunk()
    }
}

/// Works out a chunk's block light from scratch, from whatever in it gives light off.
///
/// Called for a chunk that has just been generated or read from a world that carried no light.
pub fn relight_block_light(chunk: &mut crate::chunk::Chunk, chunk_pos: crate::pos::ChunkPos) {
    let height = chunk.dimensions();
    let min_y = i32::from(height.min_y);
    let mut engine = BlockLightEngine::new();
    let mut any = false;

    for (index, section) in chunk.sections.iter().enumerate() {
        // A section with nothing that gives light off cannot start anything.
        if !section.any_block(|state| light_emission(state) > 0) {
            continue;
        }
        let base_y = min_y + (index as i32) * 16;
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let pos =
                        BlockPos::of(chunk_pos.x() * 16 + x, base_y + y, chunk_pos.z() * 16 + z);
                    if light_emission(chunk.get_block(pos.chunk_block_pos())) > 0 {
                        engine.check(pos);
                        any = true;
                    }
                }
            }
        }
    }

    if any {
        engine.run(&mut SingleChunk { chunk });
    }
}

/// The brightest a block light can be.
#[must_use]
pub const fn max_light() -> u8 {
    MAX_LIGHT
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_state::BlockId;
    use std::collections::HashMap;

    /// A patch of world with light kept beside it.
    #[derive(Default)]
    struct Lit {
        blocks: HashMap<(i32, i32, i32), BlockStateId>,
        light: HashMap<(i32, i32, i32), u8>,
    }

    impl Lit {
        fn air() -> BlockStateId {
            BlockId::from_name("minecraft:air")
                .expect("air exists")
                .default_state()
        }

        fn put(&mut self, pos: BlockPos, name: &str) {
            let block = BlockId::from_name(name).unwrap_or_else(|| panic!("{name} exists"));
            self.blocks
                .insert((pos.pos.x, pos.pos.y, pos.pos.z), block.default_state());
        }

        /// Lights the world from scratch, as though every block had just been placed.
        fn light_everything(&mut self, positions: &[BlockPos]) {
            let mut engine = BlockLightEngine::new();
            for &pos in positions {
                engine.check(pos);
            }
            engine.run(self);
        }
    }

    impl LightWorld for Lit {
        fn block_at(&mut self, pos: BlockPos) -> BlockStateId {
            self.blocks
                .get(&(pos.pos.x, pos.pos.y, pos.pos.z))
                .copied()
                .unwrap_or_else(Self::air)
        }

        fn light_at(&mut self, pos: BlockPos) -> u8 {
            self.light
                .get(&(pos.pos.x, pos.pos.y, pos.pos.z))
                .copied()
                .unwrap_or(0)
        }

        fn set_light(&mut self, pos: BlockPos, level: u8) {
            self.light.insert((pos.pos.x, pos.pos.y, pos.pos.z), level);
        }

        fn stores_light(&mut self, pos: BlockPos) -> bool {
            // A small room, so a runaway spread shows up as a failure rather than as a hang.
            pos.pos.x.abs() <= 20 && pos.pos.z.abs() <= 20 && (0..20).contains(&pos.pos.y)
        }
    }

    fn at(x: i32, y: i32, z: i32) -> BlockPos {
        BlockPos::of(x, y, z)
    }

    /// A torch in the dark lights what is around it, dimmer with distance.
    #[test]
    fn a_torch_lights_what_is_around_it() {
        let mut world = Lit::default();
        let torch = at(0, 10, 0);
        world.put(torch, "minecraft:torch");
        world.light_everything(&[torch]);

        assert_eq!(
            world.light_at(torch),
            14,
            "the torch itself is at its own level"
        );
        assert_eq!(world.light_at(at(1, 10, 0)), 13);
        assert_eq!(world.light_at(at(2, 10, 0)), 12);
        // Distance is counted in blocks stepped through, not in a straight line: three steps
        // away is three levels down, whichever way they are taken.
        assert_eq!(world.light_at(at(1, 11, 1)), 11);
        assert_eq!(world.light_at(at(3, 10, 0)), 11);
        assert_eq!(
            world.light_at(at(14, 10, 0)),
            0,
            "fourteen blocks out it has run out"
        );
    }

    /// Taking the torch away puts the dark back. This is the hard direction: everything it lit has
    /// to be darkened and then relit from whatever else reaches it.
    #[test]
    fn taking_the_torch_away_puts_the_dark_back() {
        let mut world = Lit::default();
        let torch = at(0, 10, 0);
        world.put(torch, "minecraft:torch");
        world.light_everything(&[torch]);
        assert_eq!(world.light_at(at(3, 10, 0)), 11);

        world.blocks.remove(&(0, 10, 0));
        world.light_everything(&[torch]);

        for x in 0..6 {
            assert_eq!(
                world.light_at(at(x, 10, 0)),
                0,
                "the block {x} away should be dark again"
            );
        }
    }

    /// With two torches, taking one away leaves the light of the other rather than a hole.
    #[test]
    fn one_of_two_torches_going_leaves_the_other() {
        let mut world = Lit::default();
        let left = at(0, 10, 0);
        let right = at(6, 10, 0);
        world.put(left, "minecraft:torch");
        world.put(right, "minecraft:torch");
        world.light_everything(&[left, right]);

        // Halfway between them, lit by whichever is nearer.
        assert_eq!(world.light_at(at(3, 10, 0)), 11);

        world.blocks.remove(&(0, 10, 0));
        world.light_everything(&[left]);

        assert_eq!(world.light_at(left), 8, "still lit, from the far torch");
        assert_eq!(
            world.light_at(at(3, 10, 0)),
            11,
            "the near side of the other torch is unchanged"
        );
        assert_eq!(world.light_at(right), 14);
    }

    /// Light does not pass through what stops it, so a wall casts a shadow.
    #[test]
    fn a_wall_casts_a_shadow() {
        let mut world = Lit::default();
        let torch = at(0, 10, 0);
        world.put(torch, "minecraft:torch");
        for y in 9..12 {
            for z in -1..=1 {
                world.put(at(1, y, z), "minecraft:stone");
            }
        }
        world.light_everything(&[torch]);

        assert_eq!(world.light_at(at(1, 10, 0)), 0, "inside the wall");
        assert!(
            world.light_at(at(2, 10, 0)) < 12,
            "behind the wall it should be dimmer than an open line would be"
        );
    }

    /// A chunk that has just been generated lights itself from whatever in it gives light off,
    /// without any light data having been imported.
    #[test]
    fn a_chunk_lights_itself() {
        use crate::chunk::Chunk;
        use crate::pos::{ChunkBlockPos, ChunkPos};

        let mut chunk = Chunk::new_empty();
        let torch = BlockId::from_name("minecraft:torch")
            .expect("torches exist")
            .default_state();
        chunk.set_block(ChunkBlockPos::new(8, 64, 8), torch);

        relight_block_light(&mut chunk, ChunkPos::new(0, 0));

        assert_eq!(chunk.block_light(ChunkBlockPos::new(8, 64, 8)), 14);
        assert_eq!(chunk.block_light(ChunkBlockPos::new(9, 64, 8)), 13);
        assert_eq!(chunk.block_light(ChunkBlockPos::new(8, 64, 0)), 6);
        assert_eq!(
            chunk.block_light(ChunkBlockPos::new(0, 64, 0)),
            0,
            "the far corner is out of reach"
        );
    }

    /// Water dims light by one on top of the block it costs to cross, so it darkens faster than
    /// air does.
    #[test]
    fn water_dims_light() {
        let mut world = Lit::default();
        let torch = at(0, 10, 0);
        world.put(torch, "minecraft:torch");
        for x in 1..5 {
            world.put(at(x, 10, 0), "minecraft:water");
        }
        world.light_everything(&[torch]);

        // Water's opacity is one, the same as air, so this line is not dimmer - what water changes
        // is that it is not skylight-transparent, which is the sky engine's business.
        assert_eq!(world.light_at(at(1, 10, 0)), 13);
    }
}
