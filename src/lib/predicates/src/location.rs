//! Matching a place: where it is, what is there, and what the sky is doing above it.

use crate::block::BlockPredicate;
use crate::bounds::Bounds;
use crate::context::{LootWorld, Origin};
use ferrumc_datapack::tag::TagRegistry;
use ferrumc_world::light::LightLayer;
use serde_json::Value;

/// Vanilla's `LocationPredicate`.
///
/// Biomes, structures and campfire smoke are read and never match: none of the three exists yet,
/// and vanilla fails a location whose surroundings it cannot see either.
#[derive(Clone, Debug, Default)]
pub struct LocationPredicate {
    pub x: Bounds,
    pub y: Bounds,
    pub z: Bounds,
    pub dimension: Option<String>,
    pub light: Bounds,
    pub block: Option<BlockPredicate>,
    pub can_see_sky: Option<bool>,
    /// Whether the file asked about something that cannot be answered yet.
    asks_the_unanswerable: bool,
}

impl LocationPredicate {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let position = object.get("position");
        let bound =
            |name: &str| position.map_or(Bounds::ANY, |position| Bounds::field(position, name));
        Some(Self {
            x: bound("x"),
            y: bound("y"),
            z: bound("z"),
            dimension: object
                .get("dimension")
                .and_then(Value::as_str)
                .map(str::to_owned),
            light: object
                .get("light")
                .map_or(Bounds::ANY, |light| Bounds::field(light, "light")),
            block: object.get("block").and_then(BlockPredicate::parse),
            can_see_sky: object.get("can_see_sky").and_then(Value::as_bool),
            asks_the_unanswerable: ["biomes", "structures", "smokey", "fluid"]
                .iter()
                .any(|field| object.contains_key(*field)),
        })
    }

    #[must_use]
    pub fn matches(&self, world: &dyn LootWorld, tags: &TagRegistry, at: Origin) -> bool {
        if self.asks_the_unanswerable {
            return false;
        }
        if !self.x.matches(at.x) || !self.y.matches(at.y) || !self.z.matches(at.z) {
            return false;
        }
        if let Some(dimension) = &self.dimension {
            if world.dimension() != dimension {
                return false;
            }
        }
        let pos = at.block();
        if !self.light.is_any() {
            // Vanilla's `LightPredicate` reads the block light the client would render, which is
            // the brighter of the two layers.
            let block = world.light(pos, LightLayer::Block);
            let sky = world.light(pos, LightLayer::Sky);
            let Some(brightest) = block.max(sky) else {
                return false;
            };
            if !self.light.matches(f64::from(brightest)) {
                return false;
            }
        }
        if let Some(block) = &self.block {
            if !block.matches(world, tags, pos) {
                return false;
            }
        }
        if let Some(expected) = self.can_see_sky {
            if world.can_see_sky(pos) != Some(expected) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrumc_world::block_state::BlockId;
    use ferrumc_world::block_state_id::BlockStateId;
    use ferrumc_world::pos::BlockPos;

    /// A world of one block, which is as much as a location predicate reads.
    struct OneBlock {
        state: BlockStateId,
        light: u8,
        sky: bool,
    }

    impl LootWorld for OneBlock {
        fn block_state(&self, _pos: BlockPos) -> Option<BlockStateId> {
            Some(self.state)
        }
        fn light(&self, _pos: BlockPos, _layer: LightLayer) -> Option<u8> {
            Some(self.light)
        }
        fn can_see_sky(&self, _pos: BlockPos) -> Option<bool> {
            Some(self.sky)
        }
        fn dimension(&self) -> &str {
            "minecraft:overworld"
        }
        fn time(&self) -> i64 {
            0
        }
        fn is_raining(&self) -> bool {
            false
        }
        fn is_thundering(&self) -> bool {
            false
        }
    }

    fn world() -> OneBlock {
        OneBlock {
            state: BlockId::from_name("minecraft:stone")
                .expect("stone exists")
                .default_state(),
            light: 7,
            sky: true,
        }
    }

    fn at(y: f64) -> Origin {
        Origin { x: 0.0, y, z: 0.0 }
    }

    #[test]
    fn a_position_bound_is_read_from_the_position_field() {
        let tags = ferrumc_registry::tags::current().block();
        let predicate =
            LocationPredicate::parse(&serde_json::json!({"position": {"y": {"max": 63}}}))
                .expect("a valid predicate");
        assert!(predicate.matches(&world(), &tags, at(10.0)));
        assert!(!predicate.matches(&world(), &tags, at(70.0)));
    }

    #[test]
    fn the_block_at_the_place_has_to_match() {
        let tags = ferrumc_registry::tags::current().block();
        let predicate =
            LocationPredicate::parse(&serde_json::json!({"block": {"blocks": "minecraft:stone"}}))
                .expect("a valid predicate");
        assert!(predicate.matches(&world(), &tags, at(0.0)));

        let predicate =
            LocationPredicate::parse(&serde_json::json!({"block": {"blocks": "minecraft:dirt"}}))
                .expect("a valid predicate");
        assert!(!predicate.matches(&world(), &tags, at(0.0)));
    }

    #[test]
    fn light_and_sky_are_read_from_the_world() {
        let tags = ferrumc_registry::tags::current().block();
        let dim = LocationPredicate::parse(&serde_json::json!({"light": {"light": {"max": 7}}}))
            .expect("a valid predicate");
        assert!(dim.matches(&world(), &tags, at(0.0)));

        let bright = LocationPredicate::parse(&serde_json::json!({"light": {"light": {"min": 8}}}))
            .expect("a valid predicate");
        assert!(!bright.matches(&world(), &tags, at(0.0)));

        let open = LocationPredicate::parse(&serde_json::json!({"can_see_sky": true}))
            .expect("a valid predicate");
        assert!(open.matches(&world(), &tags, at(0.0)));
        let mut underground = world();
        underground.sky = false;
        assert!(!open.matches(&underground, &tags, at(0.0)));
    }

    #[test]
    fn the_dimension_has_to_match() {
        let tags = ferrumc_registry::tags::current().block();
        let predicate =
            LocationPredicate::parse(&serde_json::json!({"dimension": "minecraft:the_nether"}))
                .expect("a valid predicate");
        assert!(!predicate.matches(&world(), &tags, at(0.0)));
    }
}
