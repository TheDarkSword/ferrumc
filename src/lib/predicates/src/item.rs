//! Matching an item: which item it is, how many, and what it carries.

use crate::bounds::Bounds;
use crate::context::ItemRef;
use crate::holders::HolderSet;
use ferrumc_datapack::tag::TagRegistry;
use serde_json::Value;

/// Vanilla's `ItemPredicate`.
#[derive(Clone, Debug, Default)]
pub struct ItemPredicate {
    pub items: Option<HolderSet>,
    pub count: Bounds,
    /// Whether the file asked about the item's components — its enchantments, its damage, its
    /// name. Nothing carries any yet, so such a predicate never matches: right for a plain tool,
    /// wrong for an enchanted one, and an enchanted one cannot exist yet.
    asks_about_components: bool,
}

impl ItemPredicate {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        Some(Self {
            items: object.get("items").and_then(HolderSet::parse),
            count: Bounds::field(value, "count"),
            asks_about_components: object.contains_key("components")
                || object.contains_key("predicates"),
        })
    }

    #[must_use]
    pub fn matches(&self, tags: &TagRegistry, item: ItemRef) -> bool {
        if self.asks_about_components {
            return false;
        }
        if let Some(items) = &self.items {
            let Ok(id) = u32::try_from(item.id) else {
                return false;
            };
            let Some(name) = ferrumc_registry::lookup_item_name(item.id) else {
                return false;
            };
            if !items.contains(tags, id, name) {
                return false;
            }
        }
        self.count.matches(f64::from(item.count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str) -> ItemRef {
        ItemRef {
            id: ferrumc_registry::lookup_item_protocol_id(name)
                .unwrap_or_else(|| panic!("{name} should be an item")),
            count: 1,
        }
    }

    #[test]
    fn matches_a_named_item() {
        let tags = ferrumc_registry::tags::current().item();
        let predicate = ItemPredicate::parse(&serde_json::json!({"items": "minecraft:shears"}))
            .expect("a valid predicate");
        assert!(predicate.matches(&tags, item("minecraft:shears")));
        assert!(!predicate.matches(&tags, item("minecraft:stone")));
    }

    #[test]
    fn matches_a_tag_of_items() {
        let tags = ferrumc_registry::tags::current().item();
        let predicate = ItemPredicate::parse(&serde_json::json!({"items": "#minecraft:planks"}))
            .expect("a valid predicate");
        assert!(predicate.matches(&tags, item("minecraft:oak_planks")));
        assert!(!predicate.matches(&tags, item("minecraft:stone")));
    }

    #[test]
    fn a_count_narrows_the_match() {
        let tags = ferrumc_registry::tags::current().item();
        let predicate = ItemPredicate::parse(&serde_json::json!({"count": {"min": 2}}))
            .expect("a valid predicate");
        assert!(!predicate.matches(&tags, item("minecraft:stone")));
        let mut two = item("minecraft:stone");
        two.count = 2;
        assert!(predicate.matches(&tags, two));
    }

    /// Until an item can carry an enchantment, asking whether it does is answered with no.
    #[test]
    fn a_component_matcher_never_matches_yet() {
        let tags = ferrumc_registry::tags::current().item();
        let predicate = ItemPredicate::parse(&serde_json::json!({
            "predicates": {"minecraft:enchantments": [{"enchantments": "minecraft:silk_touch"}]}
        }))
        .expect("a valid predicate");
        assert!(!predicate.matches(&tags, item("minecraft:diamond_pickaxe")));
    }
}
