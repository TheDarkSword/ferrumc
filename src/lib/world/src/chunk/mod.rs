pub mod heightmap;
pub mod light;
pub mod network;
mod palette;
pub mod remap;
pub mod section;

use crate::block_state_id::BlockStateId;
use crate::chunk::heightmap::Heightmaps;
use crate::chunk::section::{ChunkSection, AIR};
use crate::errors::WorldError;
use crate::pos::{BlockPos, ChunkBlockPos, ChunkHeight};
use crate::vanilla_chunk_format::VanillaChunk;
use crate::World;
use bitcode_derive::{Decode, Encode};
use deepsize::DeepSizeOf;
use ferrumc_macros::block;

#[derive(Clone, DeepSizeOf, Encode, Decode)]
pub struct Chunk {
    pub sections: Box<[ChunkSection]>,
    height: ChunkHeight,

    heightmaps: Option<Heightmaps>,

    /// What the blocks in this chunk hold beyond their state ids. Few enough per chunk that a list
    /// is faster to walk than a map is to hash, and it is written with the chunk.
    block_entities: Vec<crate::block_entity::BlockEntity>,

    /// Turns blocks in this chunk are waiting for, held here only while the chunk is not loaded.
    /// A loaded chunk's ticks live in the scheduler, where they can be ordered against every other
    /// chunk's.
    scheduled_ticks: Vec<crate::scheduler::SavedTick>,

    /// Whether this chunk has already been given the animals it is born with.
    ///
    /// A chunk is populated once, when it is first made. Without a mark that says so, a restart
    /// would hand every chunk a second herd, and the world would fill up with cows.
    populated: bool,
}

impl Chunk {
    /// Returns a chunk that is completely filled with air.
    ///
    /// This uses the overworld [`ChunkHeight`] (-64..320) as the chunk's height.
    ///
    /// # Returns
    ///
    /// * An empty chunk filled with air using the overworld [`ChunkHeight`].
    pub fn new_empty() -> Chunk {
        Self::new_empty_with_height(ChunkHeight::new(-64, 384))
    }

    /// Returns a chunk that is completely filled with air.
    ///
    /// # Arguments
    ///
    /// * `height` - The [`ChunkHeight`] that this chunk should be set to
    ///
    /// # Returns
    ///
    /// * An empty chunk filled with air using the given [`ChunkHeight`].
    pub fn new_empty_with_height(height: ChunkHeight) -> Chunk {
        Self {
            sections: vec![ChunkSection::new_uniform(AIR); (height.height / 16) as usize]
                .into_boxed_slice(),
            height,
            heightmaps: None,
            populated: false,
            block_entities: Vec::new(),
            scheduled_ticks: Vec::new(),
        }
    }

    /// Creates a chunk using the given sections and height.
    ///
    /// # Arguments
    ///
    /// * `sections` - The sections to fill the chunk with. These should be in order from the bottom of the world at index 0 and the top at the end of the slice.
    /// * `height` - The [`ChunkHeight`] to use.
    ///
    /// # Asserts
    ///
    /// * debug_assert_eq: `sections` contains enough [`ChunkSection`]s to fill the chunk based on the given [`ChunkHeight`].
    ///
    /// # Returns
    ///
    /// * A chunk using the given sections and [`ChunkHeight`]
    pub fn new_with_sections(sections: &[ChunkSection], height: ChunkHeight) -> Chunk {
        debug_assert_eq!(height.height as usize / 16, sections.len());

        Self {
            sections: sections.to_vec().into_boxed_slice(),
            height,
            heightmaps: None,
            populated: false,
            block_entities: Vec::new(),
            scheduled_ticks: Vec::new(),
        }
    }

    /// Fills an entire [`ChunkSection`] with the given block.
    ///
    /// # Arguments
    ///
    /// * `y` - The y of the section to fill.
    /// * `state` - The [`BlockStateId`] to fill the section with.
    ///
    /// # Asserts
    ///
    /// * `assert` - Checks if the given y value is in range of the height of the chunk.
    pub fn fill_section(&mut self, y: i8, state: BlockStateId) {
        assert!(y as i16 >= self.height.min_y / 16);
        assert!((y as i16) < (self.height.min_y + self.height.height as i16) / 16);

        let section = y as i16 + -self.height.min_y / 16;

        self.sections[section as usize] = ChunkSection::new_uniform(state)
    }

    /// Fills the entire chunk with the given block.
    ///
    /// # Arguments
    ///
    /// * `state` - The [`BlockStateId`] of the block to fill the chunk with.
    pub fn fill(&mut self, state: BlockStateId) {
        for section in &mut self.sections {
            *section = ChunkSection::new_uniform(state);
        }
    }

    /// Returns the chunk's vertical extent (minimum Y and total height).
    pub fn dimensions(&self) -> ChunkHeight {
        self.height
    }

    /// Gets a block in the chunk.
    ///
    /// # Arguments
    ///
    /// * `pos` - The position of the block to get.
    ///
    /// # Returns
    ///
    /// * The [`BlockStateId`] of the block at the requested position. If the position is above the maximum y of the chunk, air is always returned.
    ///   If the position is below the minimum y of the chunk, void air is always returned.
    pub fn get_block(&self, pos: ChunkBlockPos) -> BlockStateId {
        let section = (pos.y() + -self.height.min_y) / 16;
        if section < 0 {
            return block!("void_air");
        }

        if section as usize >= self.sections.len() {
            return block!("air");
        }

        self.sections[section as usize].get_block(pos.section_block_pos())
    }

    /// Turns this chunk is holding for whenever it is loaded again.
    #[must_use]
    pub fn scheduled_ticks(&self) -> &[crate::scheduler::SavedTick] {
        &self.scheduled_ticks
    }

    /// Hands its waiting turns to whoever is loading it, leaving none behind.
    pub fn take_scheduled_ticks(&mut self) -> Vec<crate::scheduler::SavedTick> {
        std::mem::take(&mut self.scheduled_ticks)
    }

    /// Gives it turns to hold while it is not loaded.
    pub fn hold_scheduled_ticks(&mut self, ticks: Vec<crate::scheduler::SavedTick>) {
        self.scheduled_ticks = ticks;
    }

    /// Everything in this chunk that holds more than its state id.
    #[must_use]
    pub fn block_entities(&self) -> &[crate::block_entity::BlockEntity] {
        &self.block_entities
    }

    /// What the block at this position holds, if anything.
    #[must_use]
    pub fn block_entity(&self, pos: ChunkBlockPos) -> Option<&crate::block_entity::BlockEntity> {
        self.block_entities
            .iter()
            .find(|entity| entity.pos() == pos)
    }

    /// The same, to be changed: a sign being written on, a furnace burning down.
    pub fn block_entity_mut(
        &mut self,
        pos: ChunkBlockPos,
    ) -> Option<&mut crate::block_entity::BlockEntity> {
        self.block_entities
            .iter_mut()
            .find(|entity| entity.pos() == pos)
    }

    /// Puts one there, replacing whatever was.
    pub fn set_block_entity(&mut self, entity: crate::block_entity::BlockEntity) {
        let pos = entity.pos();
        self.block_entities.retain(|existing| existing.pos() != pos);
        self.block_entities.push(entity);
    }

    /// Takes away whatever was there.
    pub fn remove_block_entity(&mut self, pos: ChunkBlockPos) {
        self.block_entities.retain(|entity| entity.pos() != pos);
    }

    /// The block light at a position in this chunk.
    #[must_use]
    pub fn block_light(&self, pos: ChunkBlockPos) -> u8 {
        let section = (pos.y() + -self.height.min_y) / 16;
        let Some(section) = self.sections.get(section as usize) else {
            return 0;
        };
        section
            .light
            .block_light(pos.x(), (pos.y().rem_euclid(16)) as u8, pos.z())
    }

    /// The sky light at a position in this chunk.
    #[must_use]
    pub fn sky_light(&self, pos: ChunkBlockPos) -> u8 {
        let section = (pos.y() + -self.height.min_y) / 16;
        let Some(section) = self.sections.get(section as usize) else {
            return 0;
        };
        section
            .light
            .sky_light(pos.x(), (pos.y().rem_euclid(16)) as u8, pos.z())
    }

    /// Puts one whole section's sky light at a single level.
    pub fn fill_section_sky_light(&mut self, section: usize, level: u8) {
        if let Some(section) = self.sections.get_mut(section) {
            section.light.fill_sky_light(level);
        }
    }

    pub fn set_sky_light(&mut self, pos: ChunkBlockPos, level: u8) {
        let section = (pos.y() + -self.height.min_y) / 16;
        let Some(section) = self.sections.get_mut(section as usize) else {
            return;
        };
        section
            .light
            .set_sky_light(pos.x(), (pos.y().rem_euclid(16)) as u8, pos.z(), level);
    }

    pub fn set_block_light(&mut self, pos: ChunkBlockPos, level: u8) {
        let section = (pos.y() + -self.height.min_y) / 16;
        let Some(section) = self.sections.get_mut(section as usize) else {
            return;
        };
        section
            .light
            .set_block_light(pos.x(), (pos.y().rem_euclid(16)) as u8, pos.z(), level);
    }

    /// Sets every section in this chunk to a single uniform biome.
    ///
    /// `BiomeData::Mixed` network encoding is not yet implemented, so all biome assignment
    /// must go through the `Uniform` path. For world generation this is fine: noise is smooth
    /// enough that the dominant biome at the chunk level is correct for the vast majority of
    /// columns in that chunk.
    pub fn fill_biome(&mut self, biome_id: u8) {
        use crate::chunk::section::biome::BiomeType;
        for section in self.sections.iter_mut() {
            section.biome.fill_biome(BiomeType(biome_id));
        }
    }

    /// Whether this chunk has already been given the animals it is born with.
    #[must_use]
    pub const fn populated(&self) -> bool {
        self.populated
    }

    /// Marks it as having been, so a later run does not do it again.
    pub const fn mark_populated(&mut self) {
        self.populated = true;
    }

    /// The biome at a position.
    #[must_use]
    pub fn get_biome(&self, pos: ChunkBlockPos) -> crate::chunk::section::biome::BiomeType {
        let section = (pos.y() + -self.height.min_y) / 16;
        if section < 0 || section as usize >= self.sections.len() {
            return crate::chunk::section::biome::BiomeType(0);
        }
        self.sections[section as usize]
            .biome
            .get_biome(pos.section_block_pos())
    }

    /// How high the first thing that stops movement is in a column, counting from the top.
    ///
    /// A chunk that came from a vanilla world carries this already; one this server generated does
    /// not, and there is nothing to do but look. Callers that ask often should remember the answer.
    #[must_use]
    pub fn surface_height(&self, x: u8, z: u8) -> i32 {
        if let Some(heightmaps) = self.heightmaps.as_ref() {
            return i32::from(heightmaps.motion_blocking.get_height(x, z));
        }
        let bottom = i32::from(self.height.min_y);
        let top = bottom + (self.sections.len() as i32) * 16 - 1;
        for y in (bottom..=top).rev() {
            let at = self.get_block(ChunkBlockPos::new(x, y as i16, z));
            if at != block!("air") && at != block!("void_air") {
                return y + 1;
            }
        }
        bottom
    }

    /// Sets a block in the chunk.
    ///
    /// # Arguments
    ///
    /// * `pos` - The position of the block to set within the chunk.
    /// * `id` - The [`BlockStateId`] of the block to set.
    ///
    /// # Asserts
    ///
    /// * `assert` - Checks to ensure that the given position is in-bounds.
    pub fn set_block(&mut self, pos: ChunkBlockPos, id: BlockStateId) {
        let section = (pos.y() + -self.height.min_y) / 16;
        assert!(section >= 0);
        assert!(section as usize <= self.sections.len());

        // A block that holds more than its state id gains or loses that when it is placed or
        // broken. Replacing a chest with another chest keeps what was in it, which is what happens
        // when a chest is waterlogged or turned; replacing it with anything else does not.
        let wanted = id
            .block()
            .and_then(|block| crate::block_entity::BlockEntity::for_block(block, pos));
        match (wanted, self.block_entity(pos).map(|existing| existing.kind)) {
            (Some(wanted), Some(kind)) if wanted.kind == kind => {}
            (Some(wanted), _) => self.set_block_entity(wanted),
            (None, Some(_)) => self.remove_block_entity(pos),
            (None, None) => {}
        }

        self.sections[section as usize].set_block(pos.section_block_pos(), id);
    }
}

impl TryFrom<&VanillaChunk> for Chunk {
    type Error = WorldError;

    fn try_from(value: &VanillaChunk) -> Result<Self, Self::Error> {
        let mut sections = vec![ChunkSection::new_uniform(AIR); 24];

        if value.status != "minecraft:full" {
            return Err(WorldError::CorruptedChunkData(0, 0));
        }

        for section in value
            .sections
            .as_ref()
            .ok_or(WorldError::CorruptedChunkData(
                value.x_pos as _,
                value.z_pos as _,
            ))?
            .iter()
        {
            sections[(section.y + 4).clamp(0, 23) as usize] = ChunkSection::try_from(section)?;
        }

        Ok(Chunk {
            sections: sections.into_boxed_slice(),
            height: ChunkHeight::new(-64, 384),
            heightmaps: value
                .heightmaps
                .as_ref()
                .and_then(|v| Heightmaps::try_from(v).ok()),
            // An imported world's block entities are not read yet; see
            // `internal_docs/deferred.md`.
            block_entities: Vec::new(),
            scheduled_ticks: Vec::new(),
            // An imported world already has its animals in it, saved beside the chunk.
            populated: true,
        })
    }
}

impl World {
    /// Retrieves the block data at the specified coordinates in the given dimension.
    /// Under the hood, this function just fetches the chunk containing the block and then calls
    /// [`Chunk::get_block`] on it.
    ///
    /// # Arguments
    ///
    /// * `x` - The x-coordinate of the block.
    /// * `y` - The y-coordinate of the block.
    /// * `z` - The z-coordinate of the block.
    /// * `dimension` - The dimension in which the block is located.
    ///
    /// # Returns
    ///
    /// * `Ok(BlockData)` - The block data at the specified coordinates.
    /// * `Err(WorldError)` - If an error occurs while retrieving the block data.
    ///
    /// # Errors
    ///
    /// * `WorldError::SectionOutOfBounds` - If the section containing the block is out of bounds.
    /// * `WorldError::ChunkNotFound` - If the chunk or block data is not found.
    /// * `WorldError::InvalidBlockStateData` - If the block state data is invalid.
    pub fn get_block_and_fetch(
        &self,
        pos: BlockPos,
        dimension: &str,
    ) -> Result<BlockStateId, WorldError> {
        let chunk = self.load_chunk(pos.chunk(), dimension)?;
        Ok(chunk.get_block(pos.chunk_block_pos()))
    }

    /// Sets the block data at the specified coordinates in the given dimension.
    /// Under the hood, this function just fetches the chunk containing the block and then calls
    /// [`Chunk::set_block`] on it.
    ///
    /// # Arguments
    ///
    /// * `x` - The x-coordinate of the block.
    /// * `y` - The y-coordinate of the block.
    /// * `z` - The z-coordinate of the block.
    /// * `dimension` - The dimension in which the block is located.
    /// * `block` - The block data to set.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the block data is successfully set.
    /// * `Err(WorldError)` - If an error occurs while setting the block data.
    pub fn set_block_and_fetch(
        &self,
        pos: BlockPos,
        dimension: &str,
        block: BlockStateId,
    ) -> Result<(), WorldError> {
        let mut chunk = self.load_chunk_mut(pos.chunk(), dimension)?;
        chunk.set_block(pos.chunk_block_pos(), block);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::chunk::{BlockStateId, Chunk, ChunkBlockPos};
    use ferrumc_macros::block;
    use rayon::prelude::*;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn test_read_write() {
        let mut chunk = Chunk::new_empty();

        chunk.set_block(ChunkBlockPos::new(0, 0, 0), block!("stone"));
        chunk.set_block(ChunkBlockPos::new(0, 16, 1), block!("dirt"));

        assert_eq!(
            chunk.get_block(ChunkBlockPos::new(0, 0, 0)),
            block!("stone")
        );
        assert_eq!(
            chunk.get_block(ChunkBlockPos::new(0, 16, 1)),
            block!("dirt")
        );
    }

    #[test]
    #[ignore]
    fn test_memory() {
        let now = Instant::now();

        let _chunks: Vec<_> = (0..16_000)
            .par_bridge()
            .map(|v| {
                println!("generating chunk {}", v);
                let mut chunk = Chunk::new_empty();

                for x in 0..16 {
                    for z in 0..16 {
                        for y in -64..70 {
                            chunk.set_block(ChunkBlockPos::new(x, y, z), block!("stone"));
                        }
                    }
                }

                chunk
            })
            .collect();

        println!("done. time elapsed: {:?}", now.elapsed());

        thread::sleep(Duration::from_secs(30))
    }
}
