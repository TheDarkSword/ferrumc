//! The worldgen definitions the game ships as json.
//!
//! Biomes, features, the places they go and the structures that are built: all of it is datapack
//! json in modern Minecraft, and all of it is read here into types a generator can run. This crate
//! is the reading; running them is worldgen's own.

pub mod biome;
pub mod feature;
pub mod holders;
pub mod placement;
pub mod predicate;
pub mod state;
pub mod structure;
pub mod tree;
pub mod value;

pub use biome::Biome;
pub use feature::ConfiguredFeature;
pub use holders::IdSet;
pub use placement::{PlacedFeature, PlacementModifier};
pub use predicate::BlockPredicate;
pub use state::{parse_block_state, BlockStateProvider, RuleTest};
pub use structure::{Structure, StructureSet};
pub use value::{FloatProvider, HeightProvider, IntProvider, VerticalAnchor};

#[cfg(test)]
mod tests;

use ferrumc_datapack::manager::FileToId;
use ferrumc_datapack::{Identifier, ResourceManager};
use serde_json::Value;
use std::collections::BTreeMap;
use tracing::error;

/// Every worldgen definition the loaded packs declare.
#[derive(Debug, Default)]
pub struct WorldgenData {
    pub biomes: BTreeMap<String, Biome>,
    pub configured_features: BTreeMap<String, ConfiguredFeature>,
    pub placed_features: BTreeMap<String, PlacedFeature>,
    pub structures: BTreeMap<String, Structure>,
    pub structure_sets: BTreeMap<String, StructureSet>,
}

impl WorldgenData {
    /// Reads every worldgen definition in a pack stack.
    #[must_use]
    pub fn load(manager: &ResourceManager) -> Self {
        Self {
            biomes: read(manager, "biome", Biome::parse),
            configured_features: read(manager, "configured_feature", ConfiguredFeature::parse),
            placed_features: read(manager, "placed_feature", PlacedFeature::parse),
            structures: read(manager, "structure", Structure::parse),
            structure_sets: read(manager, "structure_set", StructureSet::parse),
        }
    }

    #[must_use]
    pub fn biome(&self, name: &Identifier) -> Option<&Biome> {
        self.biomes.get(name.as_str())
    }

    #[must_use]
    pub fn placed_feature(&self, name: &Identifier) -> Option<&PlacedFeature> {
        self.placed_features.get(name.as_str())
    }

    #[must_use]
    pub fn configured_feature(&self, name: &Identifier) -> Option<&ConfiguredFeature> {
        self.configured_features.get(name.as_str())
    }

    /// How many definitions were read, for a line in the log.
    #[must_use]
    pub fn len(&self) -> usize {
        self.biomes.len()
            + self.configured_features.len()
            + self.placed_features.len()
            + self.structures.len()
            + self.structure_sets.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Reads one kind of definition, saying which files could not be read rather than stopping.
fn read<T>(
    manager: &ResourceManager,
    kind: &str,
    parse: impl Fn(&Value) -> Option<T>,
) -> BTreeMap<String, T> {
    let mut by_name = BTreeMap::new();
    for (id, resource) in FileToId::json(&format!("worldgen/{kind}")).list(manager) {
        let Ok(value) = serde_json::from_slice::<Value>(&resource.data) else {
            error!("{kind} {id} from data pack {} is not json", resource.source);
            continue;
        };
        match parse(&value) {
            Some(parsed) => {
                by_name.insert(id.as_str().to_owned(), parsed);
            }
            None => error!(
                "couldn't read {kind} {id} from data pack {}",
                resource.source
            ),
        }
    }
    by_name
}
