//! The pack stack the server reads its json out of.
//!
//! Holds the packs that were found and the stack they currently form. Everything datapack-driven
//! reads through the stack, and a reload rebuilds it from what is on disk now.

use bevy_ecs::prelude::Resource;
use ferrumc_datapack::{DatapackError, PackRepository, ResourceManager};
use ferrumc_general_purpose::paths::get_root_path;
use std::sync::Arc;
use tracing::info;

/// Where a player drops a datapack.
pub const DATAPACK_DIR: &str = "datapacks";

#[derive(Resource)]
pub struct Datapacks {
    pub repository: PackRepository,
    /// The selected packs in the order they override each other.
    pub resources: ResourceManager,
}

impl Datapacks {
    /// Reads the built-in pack and everything in the datapack directory.
    pub fn load() -> Result<Self, DatapackError> {
        let repository = PackRepository::discover(get_root_path().join(DATAPACK_DIR))?;
        let resources = repository.open();
        let packs = Self {
            repository,
            resources,
        };
        packs.rebuild();
        packs.report();
        Ok(packs)
    }

    /// Looks at the directory again and rebuilds the stack. What `/reload` runs.
    pub fn reload(&mut self) -> Result<(), DatapackError> {
        self.repository.reload()?;
        self.resources = self.repository.open();
        self.rebuild();
        self.report();
        Ok(())
    }

    /// Reads everything the packs define again. Each thing datapacks drive adds its line here.
    fn rebuild(&self) {
        let started = std::time::Instant::now();
        let block_tags = ferrumc_world::block_tag::load(&self.resources);
        let tag_count = block_tags.len();
        ferrumc_world::block_tag::set(Arc::new(block_tags));
        tracing::debug!("read {tag_count} block tags in {:.1?}", started.elapsed());
    }

    fn report(&self) {
        info!(
            "data packs loaded: {}",
            self.repository.selected().join(", ")
        );
    }
}
