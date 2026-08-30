//! The pack stack the server reads its json out of.
//!
//! Holds the packs that were found and the stack they currently form. Everything datapack-driven
//! reads through the stack, and a reload rebuilds it from what is on disk now.

use bevy_ecs::prelude::Resource;
use ferrumc_datapack::{DatapackError, PackRepository, ResourceManager};
use ferrumc_general_purpose::paths::get_root_path;
use ferrumc_loot::LootTables;
use ferrumc_predicates::Predicates;
use ferrumc_recipes::RecipeBook;
use std::sync::Arc;
use tracing::info;

/// Where a player drops a datapack.
pub const DATAPACK_DIR: &str = "datapacks";

#[derive(Resource)]
pub struct Datapacks {
    pub repository: PackRepository,
    /// The selected packs in the order they override each other.
    pub resources: ResourceManager,
    /// The predicates the packs declare, which anything gating on one names.
    pub predicates: Arc<Predicates>,
    /// Every loot table, which is where each drop in the game comes from.
    pub loot: Arc<LootTables>,
    /// Every recipe, which is what the game can be made into.
    pub recipes: Arc<RecipeBook>,
}

impl Datapacks {
    /// Reads the built-in pack and everything in the datapack directory.
    pub fn load() -> Result<Self, DatapackError> {
        let repository = PackRepository::discover(get_root_path().join(DATAPACK_DIR))?;
        let resources = repository.open();
        let mut packs = Self {
            repository,
            resources,
            predicates: Arc::default(),
            loot: Arc::default(),
            recipes: Arc::default(),
        };
        packs.rebuild();
        packs.predicates = Arc::new(Predicates::load(&packs.resources));
        packs.loot = Arc::new(LootTables::load(&packs.resources));
        packs.recipes = Arc::new(RecipeBook::load(&packs.resources));
        packs.report();
        Ok(packs)
    }

    /// Looks at the directory again and rebuilds the stack. What `/reload` runs.
    pub fn reload(&mut self) -> Result<(), DatapackError> {
        self.repository.reload()?;
        self.resources = self.repository.open();
        self.rebuild();
        self.predicates = Arc::new(Predicates::load(&self.resources));
        self.loot = Arc::new(LootTables::load(&self.resources));
        self.recipes = Arc::new(RecipeBook::load(&self.resources));
        self.report();
        Ok(())
    }

    /// Reads everything the packs define again. Each thing datapacks drive adds its line here.
    fn rebuild(&self) {
        let started = std::time::Instant::now();
        let tags = Arc::new(ferrumc_registry::tags::load(&self.resources));
        let counts: Vec<String> = tags
            .iter()
            .map(|(registry, tags)| format!("{registry} {}", tags.len()))
            .collect();
        ferrumc_net::packets::outgoing::update_tags::set(Arc::new(
            ferrumc_net::packets::outgoing::update_tags::build_packet(&tags),
        ));
        ferrumc_registry::tags::set(tags);
        tracing::debug!(
            "read tags in {:.1?}: {}",
            started.elapsed(),
            counts.join(", ")
        );
    }

    fn report(&self) {
        info!(
            "data packs loaded: {} ({} recipes, {} loot tables, {} predicates)",
            self.repository.selected().join(", "),
            self.recipes.len(),
            self.loot.len(),
            self.predicates.len()
        );
    }
}
