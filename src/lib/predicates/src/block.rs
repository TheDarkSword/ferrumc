//! Matching a block: which block it is, and what state it is in.

use crate::context::LootWorld;
use crate::holders::HolderSet;
use crate::state::StateProperties;
use ferrumc_datapack::tag::TagRegistry;
use ferrumc_world::block_state_id::BlockStateId;
use ferrumc_world::pos::BlockPos;
use serde_json::Value;

/// Vanilla's `BlockPredicate`.
///
/// The `nbt` and `components` halves are read and never match: they ask a block entity what it
/// holds, and nothing but a sign holds anything yet.
#[derive(Clone, Debug, Default)]
pub struct BlockPredicate {
    pub blocks: Option<HolderSet>,
    pub state: Option<StateProperties>,
    /// Whether the file asked about a block entity's contents.
    asks_about_contents: bool,
}

impl BlockPredicate {
    /// A predicate on a block and its state alone, which is the shape `block_state_property`
    /// writes: a bare block id and a `properties` field rather than a set and a `state` one.
    #[must_use]
    pub fn of(blocks: Option<HolderSet>, state: Option<StateProperties>) -> Self {
        Self {
            blocks,
            state,
            asks_about_contents: false,
        }
    }

    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        Some(Self {
            blocks: object.get("blocks").and_then(HolderSet::parse),
            state: object.get("state").and_then(StateProperties::parse),
            asks_about_contents: object.contains_key("nbt")
                || object.contains_key("components")
                || object.contains_key("predicates"),
        })
    }

    /// Whether the state is this block in this state, without looking at the world.
    #[must_use]
    pub fn matches_state(&self, tags: &TagRegistry, state: BlockStateId) -> bool {
        if self.asks_about_contents {
            return false;
        }
        if let Some(blocks) = &self.blocks {
            let Some(block) = state.block() else {
                return false;
            };
            if !blocks.contains(tags, u32::from(block.index()), block.name()) {
                return false;
            }
        }
        self.state
            .as_ref()
            .is_none_or(|state_predicate| state_predicate.matches(state))
    }

    /// The same, of whatever is at a position. A position that is not loaded never matches, as
    /// vanilla's `isLoaded` check gives.
    #[must_use]
    pub fn matches(&self, world: &dyn LootWorld, tags: &TagRegistry, pos: BlockPos) -> bool {
        world
            .block_state(pos)
            .is_some_and(|state| self.matches_state(tags, state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrumc_world::block_state::BlockId;

    fn state(name: &str) -> BlockStateId {
        BlockId::from_name(name)
            .unwrap_or_else(|| panic!("{name} should exist"))
            .default_state()
    }

    #[test]
    fn matches_a_named_block() {
        let tags = ferrumc_registry::tags::current().block();
        let predicate = BlockPredicate::parse(&serde_json::json!({"blocks": "minecraft:stone"}))
            .expect("a valid predicate");
        assert!(predicate.matches_state(&tags, state("minecraft:stone")));
        assert!(!predicate.matches_state(&tags, state("minecraft:dirt")));
    }

    #[test]
    fn matches_a_tag_of_blocks() {
        let tags = ferrumc_registry::tags::current().block();
        let predicate = BlockPredicate::parse(&serde_json::json!({"blocks": "#minecraft:logs"}))
            .expect("a valid predicate");
        assert!(predicate.matches_state(&tags, state("minecraft:oak_log")));
        assert!(!predicate.matches_state(&tags, state("minecraft:stone")));
    }

    #[test]
    fn a_state_narrows_the_block() {
        let tags = ferrumc_registry::tags::current().block();
        let predicate = BlockPredicate::parse(
            &serde_json::json!({"blocks": "minecraft:oak_door", "state": {"half": "lower"}}),
        )
        .expect("a valid predicate");
        let door = state("minecraft:oak_door");
        assert!(predicate.matches_state(&tags, door));

        let half = door
            .block()
            .expect("a door is a block")
            .properties()
            .find(|p| p.name() == "half")
            .expect("a door has a half");
        assert!(
            !predicate.matches_state(&tags, door.with_raw(half, "upper").expect("an upper half"))
        );
    }

    #[test]
    fn an_empty_predicate_matches_anything() {
        let tags = ferrumc_registry::tags::current().block();
        let predicate = BlockPredicate::parse(&serde_json::json!({})).expect("a valid predicate");
        assert!(predicate.matches_state(&tags, state("minecraft:stone")));
    }
}
