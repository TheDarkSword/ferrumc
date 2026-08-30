//! Tags over the registries the server carries itself, as the loaded packs declare them.
//!
//! A tag is a named set over a registry, and the registries here are the built-in ones whose
//! entries have a fixed numeric id — blocks, items, fluids, entity types, game events and points
//! of interest. Their tags are held keyed by that id, which is both what a query wants and what
//! the wire carries.
//!
//! The datapack-driven registries (biomes, damage types, enchantments) carry their tags with the
//! registry sync instead, and are not here.

use ferrumc_datapack::ResourceManager;
use ferrumc_datapack::tag::{RawTags, TagRegistry};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock, RwLock};

/// The numeric ids each registry's entries have, as this version assigns them.
const REGISTRIES_JSON: &str = include_str!("../../../../assets/data/registries.json");

/// Each registry that has tags here, paired with the directory a pack keeps them in.
pub const REGISTRIES: &[(&str, &str)] = &[
    ("minecraft:block", "tags/block"),
    ("minecraft:item", "tags/item"),
    ("minecraft:fluid", "tags/fluid"),
    ("minecraft:entity_type", "tags/entity_type"),
    ("minecraft:game_event", "tags/game_event"),
    (
        "minecraft:point_of_interest_type",
        "tags/point_of_interest_type",
    ),
];

/// Every registry's tags, keyed by registry.
pub struct GameTags {
    by_registry: BTreeMap<&'static str, Arc<TagRegistry>>,
}

impl GameTags {
    /// The tags over one registry, empty where that registry has none.
    #[must_use]
    pub fn get(&self, registry: &str) -> Arc<TagRegistry> {
        self.by_registry
            .get(registry)
            .map_or_else(|| Arc::new(TagRegistry::new(0)), Arc::clone)
    }

    #[must_use]
    pub fn block(&self) -> Arc<TagRegistry> {
        self.get("minecraft:block")
    }

    #[must_use]
    pub fn item(&self) -> Arc<TagRegistry> {
        self.get("minecraft:item")
    }

    /// Every registry and its tags, in the order [`REGISTRIES`] names them.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &Arc<TagRegistry>)> {
        REGISTRIES
            .iter()
            .filter_map(|(registry, _)| Some((*registry, self.by_registry.get(registry)?)))
    }
}

/// Reads every registry's tags out of a pack stack.
#[must_use]
pub fn load(manager: &ResourceManager) -> GameTags {
    let ids = &*REGISTRY_IDS;
    let mut by_registry = BTreeMap::new();
    for (registry, directory) in REGISTRIES {
        let Some(id_map) = ids.get(registry) else {
            continue;
        };
        // Tags resolve straight to the registry's own ids, so what a query tests and what the wire
        // carries are the same numbers.
        let element_count = id_map.values().max().map_or(0, |max| *max as usize + 1);
        let tags = RawTags::load(manager, directory).build(element_count, |id| {
            id_map
                .get(id.as_str())
                .and_then(|id| u32::try_from(*id).ok())
        });
        by_registry.insert(*registry, Arc::new(tags));
    }
    GameTags { by_registry }
}

/// The numeric id this version gives an entry of a registry, which is what tags resolve to and
/// what the wire carries.
#[must_use]
pub fn protocol_id(registry: &str, entry: &str) -> Option<i32> {
    REGISTRY_IDS.get(registry)?.get(entry).copied()
}

/// `entry name -> numeric id`, for each registry that has tags.
static REGISTRY_IDS: LazyLock<BTreeMap<&'static str, BTreeMap<String, i32>>> =
    LazyLock::new(registry_ids);

fn registry_ids() -> BTreeMap<&'static str, BTreeMap<String, i32>> {
    let registries: Value =
        serde_json::from_str(REGISTRIES_JSON).expect("registries.json should be valid json");
    let mut out = BTreeMap::new();
    for (registry, _directory) in REGISTRIES {
        let Some(entries) = registries
            .get(registry)
            .and_then(|r| r.get("entries"))
            .and_then(Value::as_object)
        else {
            continue;
        };
        let ids = entries
            .iter()
            .filter_map(|(name, info)| {
                let id = info.get("protocol_id")?.as_i64()?;
                Some((name.clone(), i32::try_from(id).ok()?))
            })
            .collect();
        out.insert(*registry, ids);
    }
    out
}

/// Falls back to the pack the server ships with, so anything asking before the datapacks are read
/// gets vanilla's answer rather than an empty one.
static CURRENT: LazyLock<RwLock<Arc<GameTags>>> = LazyLock::new(|| {
    let built_in = ferrumc_datapack::vanilla_pack()
        .map(|pack| ResourceManager::new(vec![Arc::new(pack)]))
        .map_or_else(
            |e| {
                tracing::error!("could not read the built-in tags: {e}");
                GameTags {
                    by_registry: BTreeMap::new(),
                }
            },
            |manager| load(&manager),
        );
    RwLock::new(Arc::new(built_in))
});

/// The tags as they stand.
///
/// Hold on to this for the length of a piece of work rather than calling it per element: it takes
/// a lock and clones a handle each time.
#[must_use]
pub fn current() -> Arc<GameTags> {
    CURRENT
        .read()
        .expect("the tags are never held across a panic")
        .clone()
}

/// Replaces them, which is what loading or reloading datapacks does.
pub fn set(tags: Arc<GameTags>) {
    *CURRENT
        .write()
        .expect("the tags are never held across a panic") = tags;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_pack_carries_every_registry() {
        let tags = current();
        for (registry, _) in REGISTRIES {
            let registry_tags = tags.get(registry);
            assert!(
                !registry_tags.is_empty(),
                "{registry} should carry at least one tag"
            );
        }
    }

    #[test]
    fn a_tag_holds_the_registry_ids_of_its_members() {
        let tags = current();
        let items = tags.item();
        let planks = items
            .get_by_name("minecraft:planks")
            .expect("planks are tagged");
        let oak =
            crate::lookup_item_protocol_id("minecraft:oak_planks").expect("oak planks are an item");
        assert!(items.contains(planks, u32::try_from(oak).expect("ids are not negative")));
    }
}
