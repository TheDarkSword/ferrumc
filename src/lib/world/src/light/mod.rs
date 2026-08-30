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

use crate::block_data::{
    face_occludes_light, light_dampening, light_emission, light_opacity, MAX_LIGHT,
};
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

/// The two kinds of light, kept separately because they answer different questions: one is where
/// the sun reaches, the other is what a player has lit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightLayer {
    Block,
    Sky,
}

/// What a light engine needs of the world.
pub trait LightWorld {
    fn block_at(&mut self, pos: BlockPos) -> BlockStateId;
    fn light_at(&mut self, pos: BlockPos, layer: LightLayer) -> u8;
    fn set_light(&mut self, pos: BlockPos, layer: LightLayer, level: u8);
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

/// Spreads light, and takes it back.
///
/// The spreading is the same for both kinds; what differs is where the light starts. Block light
/// starts at whatever gives it off, sky light at every position the sky reaches.
pub struct LightEngine {
    layer: LightLayer,
    to_check: Vec<BlockPos>,
    increase: VecDeque<Entry>,
    decrease: VecDeque<Entry>,
}

/// The engine that spreads what blocks give off.
pub type BlockLightEngine = LightEngine;

impl Default for LightEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LightEngine {
    #[must_use]
    pub fn new() -> Self {
        Self::for_layer(LightLayer::Block)
    }

    #[must_use]
    pub fn for_layer(layer: LightLayer) -> Self {
        Self {
            layer,
            to_check: Vec::new(),
            increase: VecDeque::new(),
            decrease: VecDeque::new(),
        }
    }

    /// Marks a position as a source of full light that spreads everywhere but upwards, which is
    /// what a position the sky reaches is: above it is another one.
    pub fn add_sky_source(&mut self, pos: BlockPos) {
        self.increase.push_back(Entry {
            pos,
            level: MAX_LIGHT,
            directions: ANY & !bit(Direction::Up),
            from_emission: true,
        });
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
        let old = world.light_at(pos, self.layer);

        if emission < old {
            // Whatever was here is gone or dimmer: take back what it gave out.
            world.set_light(pos, self.layer, 0);
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
        let mut level = world.light_at(entry.pos, self.layer);
        if entry.from_emission && level < entry.level {
            world.set_light(entry.pos, self.layer, entry.level);
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
            let to_level = world.light_at(to, self.layer);
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

            world.set_light(to, self.layer, new_level);
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
            let to_level = world.light_at(to, self.layer);
            if to_level == 0 {
                continue;
            }

            if to_level <= entry.level.saturating_sub(1) {
                // Dim enough to have come from here, so it goes too.
                let to_state = world.block_at(to);
                let to_emission = light_emission(to_state);
                world.set_light(to, self.layer, 0);
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
    at: crate::pos::ChunkPos,
}

impl LightWorld for SingleChunk<'_> {
    fn block_at(&mut self, pos: BlockPos) -> BlockStateId {
        self.chunk.get_block(pos.chunk_block_pos())
    }

    fn light_at(&mut self, pos: BlockPos, layer: LightLayer) -> u8 {
        match layer {
            LightLayer::Block => self.chunk.block_light(pos.chunk_block_pos()),
            LightLayer::Sky => self.chunk.sky_light(pos.chunk_block_pos()),
        }
    }

    fn set_light(&mut self, pos: BlockPos, layer: LightLayer, level: u8) {
        match layer {
            LightLayer::Block => self.chunk.set_block_light(pos.chunk_block_pos(), level),
            LightLayer::Sky => self.chunk.set_sky_light(pos.chunk_block_pos(), level),
        }
    }

    fn stores_light(&mut self, pos: BlockPos) -> bool {
        // Only this chunk, and only within its height. A position outside it would otherwise be
        // read and written through the chunk's own coordinates, which wrap: light would come back
        // in the far side.
        let height = self.chunk.dimensions();
        let min = i32::from(height.min_y);
        let chunk = pos.chunk();
        pos.pos.y >= min
            && pos.pos.y < min + i32::from(height.height)
            && chunk.x() == self.at.x()
            && chunk.z() == self.at.z()
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
        engine.run(&mut SingleChunk {
            chunk,
            at: chunk_pos,
        });
    }
}

/// Whether the sky stops between these two, the upper one first.
///
/// Anything that dims light at all stops the sky, and so does a pair of faces that closes the gap
/// between them: a trapdoor lets the sky past when open and not when shut.
fn sky_stops_between(top: BlockStateId, bottom: BlockStateId) -> bool {
    light_dampening(bottom) != 0
        || face_occludes_light(top, Direction::Down)
        || face_occludes_light(bottom, Direction::Up)
}

/// Works out a chunk's sky light from scratch.
///
/// Every position the sky reaches is a source at full strength — that is what makes an open column
/// bright all the way down rather than dimmer with depth — and the light then spreads sideways and
/// under overhangs from those, losing a level a block like any other.
pub fn relight_sky_light(chunk: &mut crate::chunk::Chunk, chunk_pos: crate::pos::ChunkPos) {
    let height = chunk.dimensions();
    let min_y = i32::from(height.min_y);
    let max_y = min_y + i32::from(height.height);

    let air = crate::block_state::BlockId::from_name("minecraft:air")
        .map(crate::block_state::BlockId::default_state)
        .unwrap_or_else(|| BlockStateId::new(0));

    // Where the sky stops in each column: the first position going down that it cannot reach.
    let mut lowest_source = [[min_y; 16]; 16];
    let mut deepest = min_y;
    for (x, column) in lowest_source.iter_mut().enumerate() {
        for (z, source) in column.iter_mut().enumerate() {
            let world_x = chunk_pos.x() * 16 + x as i32;
            let world_z = chunk_pos.z() * 16 + z as i32;
            let mut top = air;
            for y in (min_y..max_y).rev() {
                let pos = BlockPos::of(world_x, y, world_z);
                let bottom = chunk.get_block(pos.chunk_block_pos());
                if sky_stops_between(top, bottom) {
                    *source = y + 1;
                    break;
                }
                top = bottom;
            }
            deepest = deepest.max(*source);
        }
    }

    // Above the deepest of them every column is a source, so the whole band is already at full
    // strength and there is nothing for light to spread into. Leaving those sections alone is what
    // keeps them in their uniform form instead of spelling out a nibble per block.
    if deepest <= min_y {
        return;
    }

    // Below the shallowest, no column sees the sky at all, so those sections are dark as a whole
    // and can say so in one go rather than a nibble at a time. Between the two is the only part
    // that has to be written out position by position.
    let shallowest = lowest_source
        .iter()
        .flatten()
        .copied()
        .min()
        .unwrap_or(min_y);
    for index in 0..(i32::from(height.height) / 16) {
        let base = min_y + index * 16;
        if base + 16 <= shallowest {
            chunk.fill_section_sky_light(index as usize, 0);
        }
    }
    let explicit_from = shallowest.max(min_y) - (shallowest.max(min_y) - min_y) % 16;

    let mut engine = LightEngine::for_layer(LightLayer::Sky);
    for (x, column) in lowest_source.iter().enumerate() {
        for (z, &source) in column.iter().enumerate() {
            let world_x = chunk_pos.x() * 16 + x as i32;
            let world_z = chunk_pos.z() * 16 + z as i32;
            for y in explicit_from..deepest {
                let pos = BlockPos::of(world_x, y, world_z);
                if y >= source {
                    chunk.set_sky_light(pos.chunk_block_pos(), MAX_LIGHT);
                    engine.add_sky_source(pos);
                } else {
                    chunk.set_sky_light(pos.chunk_block_pos(), 0);
                }
            }
        }
    }

    engine.run(&mut SingleChunk {
        chunk,
        at: chunk_pos,
    });
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

        fn light_at(&mut self, pos: BlockPos, _layer: LightLayer) -> u8 {
            self.light
                .get(&(pos.pos.x, pos.pos.y, pos.pos.z))
                .copied()
                .unwrap_or(0)
        }

        fn set_light(&mut self, pos: BlockPos, _layer: LightLayer, level: u8) {
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
            world.light_at(torch, LightLayer::Block),
            14,
            "the torch itself is at its own level"
        );
        assert_eq!(world.light_at(at(1, 10, 0), LightLayer::Block), 13);
        assert_eq!(world.light_at(at(2, 10, 0), LightLayer::Block), 12);
        // Distance is counted in blocks stepped through, not in a straight line: three steps
        // away is three levels down, whichever way they are taken.
        assert_eq!(world.light_at(at(1, 11, 1), LightLayer::Block), 11);
        assert_eq!(world.light_at(at(3, 10, 0), LightLayer::Block), 11);
        assert_eq!(
            world.light_at(at(14, 10, 0), LightLayer::Block),
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
        assert_eq!(world.light_at(at(3, 10, 0), LightLayer::Block), 11);

        world.blocks.remove(&(0, 10, 0));
        world.light_everything(&[torch]);

        for x in 0..6 {
            assert_eq!(
                world.light_at(at(x, 10, 0), LightLayer::Block),
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
        assert_eq!(world.light_at(at(3, 10, 0), LightLayer::Block), 11);

        world.blocks.remove(&(0, 10, 0));
        world.light_everything(&[left]);

        assert_eq!(
            world.light_at(left, LightLayer::Block),
            8,
            "still lit, from the far torch"
        );
        assert_eq!(
            world.light_at(at(3, 10, 0), LightLayer::Block),
            11,
            "the near side of the other torch is unchanged"
        );
        assert_eq!(world.light_at(right, LightLayer::Block), 14);
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

        assert_eq!(
            world.light_at(at(1, 10, 0), LightLayer::Block),
            0,
            "inside the wall"
        );
        assert!(
            world.light_at(at(2, 10, 0), LightLayer::Block) < 12,
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

    /// An open column is bright all the way down: every position in it sees the sky, so every one
    /// of them is a source rather than one source dimming with depth.
    #[test]
    fn the_sky_reaches_the_ground_at_full_strength() {
        use crate::chunk::Chunk;
        use crate::pos::{ChunkBlockPos, ChunkPos};

        let mut chunk = Chunk::new_empty();
        let stone = BlockId::from_name("minecraft:stone")
            .expect("stone exists")
            .default_state();
        // A floor at y = 64, nothing above it.
        for x in 0..16u8 {
            for z in 0..16u8 {
                chunk.set_block(ChunkBlockPos::new(x, 64, z), stone);
            }
        }

        relight_sky_light(&mut chunk, ChunkPos::new(0, 0));

        assert_eq!(
            chunk.sky_light(ChunkBlockPos::new(8, 65, 8)),
            15,
            "just above the floor"
        );
        assert_eq!(
            chunk.sky_light(ChunkBlockPos::new(8, 200, 8)),
            15,
            "high above it"
        );
        assert_eq!(
            chunk.sky_light(ChunkBlockPos::new(8, 64, 8)),
            0,
            "inside the floor"
        );
        assert_eq!(chunk.sky_light(ChunkBlockPos::new(8, 63, 8)), 0, "under it");
    }

    /// Under an overhang the sky arrives sideways and dims with every block it crosses.
    #[test]
    fn the_sky_dims_under_an_overhang() {
        use crate::chunk::Chunk;
        use crate::pos::{ChunkBlockPos, ChunkPos};

        let mut chunk = Chunk::new_empty();
        let stone = BlockId::from_name("minecraft:stone")
            .expect("stone exists")
            .default_state();
        // A floor, and a roof over half of it.
        for x in 0..16u8 {
            for z in 0..16u8 {
                chunk.set_block(ChunkBlockPos::new(x, 64, z), stone);
            }
        }
        for x in 0..8u8 {
            for z in 0..16u8 {
                chunk.set_block(ChunkBlockPos::new(x, 70, z), stone);
            }
        }

        relight_sky_light(&mut chunk, ChunkPos::new(0, 0));

        // Out in the open, full strength.
        assert_eq!(chunk.sky_light(ChunkBlockPos::new(9, 65, 8)), 15);
        // Just inside the overhang, one level down from the open air beside it.
        assert_eq!(chunk.sky_light(ChunkBlockPos::new(7, 65, 8)), 14);
        // Further in, dimmer still.
        assert_eq!(chunk.sky_light(ChunkBlockPos::new(6, 65, 8)), 13);
        // Eight blocks in from the open edge, so eight levels down.
        assert_eq!(chunk.sky_light(ChunkBlockPos::new(0, 65, 8)), 7);
    }

    /// Glass dims nothing, so the sky carries straight through it.
    #[test]
    fn the_sky_comes_through_glass() {
        use crate::chunk::Chunk;
        use crate::pos::{ChunkBlockPos, ChunkPos};

        let mut chunk = Chunk::new_empty();
        let stone = BlockId::from_name("minecraft:stone")
            .expect("stone exists")
            .default_state();
        let glass = BlockId::from_name("minecraft:glass")
            .expect("glass exists")
            .default_state();
        for x in 0..16u8 {
            for z in 0..16u8 {
                chunk.set_block(ChunkBlockPos::new(x, 64, z), stone);
                chunk.set_block(ChunkBlockPos::new(x, 70, z), glass);
            }
        }

        relight_sky_light(&mut chunk, ChunkPos::new(0, 0));

        assert_eq!(
            chunk.sky_light(ChunkBlockPos::new(8, 65, 8)),
            15,
            "under a glass roof the sky still reaches the floor"
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
        assert_eq!(world.light_at(at(1, 10, 0), LightLayer::Block), 13);
    }
}
