//! Matching a block's properties: `{"half": "lower", "age": {"min": "5"}}`.
//!
//! Vanilla's `StatePropertiesPredicate`. Values are compared as the strings they are written as,
//! and a range compares by the property's own order — `age` runs 0 to 15 numerically, a stair's
//! `shape` by the order its values are declared in.

use ferrumc_world::block_state_id::BlockStateId;
use serde_json::Value;
use std::collections::BTreeMap;

/// What one property has to be.
#[derive(Clone, Debug)]
enum ValueMatcher {
    Exact(String),
    Between {
        min: Option<String>,
        max: Option<String>,
    },
}

/// Every property that has to match.
#[derive(Clone, Debug, Default)]
pub struct StateProperties {
    properties: BTreeMap<String, ValueMatcher>,
}

impl StateProperties {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let mut properties = BTreeMap::new();
        for (name, matcher) in object {
            let matcher = match matcher {
                Value::String(exact) => ValueMatcher::Exact(exact.clone()),
                Value::Object(range) => ValueMatcher::Between {
                    min: range.get("min").and_then(Value::as_str).map(str::to_owned),
                    max: range.get("max").and_then(Value::as_str).map(str::to_owned),
                },
                // A bare number or boolean is written as a string in vanilla's data, so anything
                // else is a file that will not do what it says.
                _ => return None,
            };
            properties.insert(name.clone(), matcher);
        }
        Some(Self { properties })
    }

    #[must_use]
    pub fn matches(&self, state: BlockStateId) -> bool {
        self.properties
            .iter()
            .all(|(name, matcher)| matches_one(state, name, matcher))
    }
}

fn matches_one(state: BlockStateId, name: &str, matcher: &ValueMatcher) -> bool {
    let Some(block) = state.block() else {
        return false;
    };
    let Some(property) = block.properties().find(|p| p.name() == name) else {
        // A property the block does not have never matches, as vanilla's null check gives.
        return false;
    };
    let Some(actual) = state.get_raw(property) else {
        return false;
    };
    match matcher {
        ValueMatcher::Exact(expected) => actual == expected,
        ValueMatcher::Between { min, max } => {
            // A range is over the property's own values in the order they are declared, which is
            // the order the ids are built in, so comparing positions compares values.
            let Some(values) = block.property_values(property) else {
                return false;
            };
            let Some(at) = values.iter().position(|value| *value == actual) else {
                return false;
            };
            let within = |bound: &Option<String>, keep: fn(usize, usize) -> bool| {
                bound.as_ref().is_none_or(|bound| {
                    values
                        .iter()
                        .position(|value| value == bound)
                        .is_some_and(|edge| keep(at, edge))
                })
            };
            within(min, |at, edge| at >= edge) && within(max, |at, edge| at <= edge)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrumc_world::block_state::{properties, BlockId};

    fn state(name: &str) -> BlockStateId {
        BlockId::from_name(name)
            .unwrap_or_else(|| panic!("{name} should exist"))
            .default_state()
    }

    #[test]
    fn an_exact_value_matches_only_itself() {
        let predicate =
            StateProperties::parse(&serde_json::json!({"half": "lower"})).expect("a valid matcher");
        let door = state("minecraft:oak_door");
        assert!(predicate.matches(door));

        let upper = door
            .with_raw(
                door.block()
                    .expect("a door is a block")
                    .properties()
                    .find(|p| p.name() == "half")
                    .expect("a door has a half"),
                "upper",
            )
            .expect("upper is a half");
        assert!(!predicate.matches(upper));
    }

    #[test]
    fn a_range_compares_by_the_property_order() {
        let predicate = StateProperties::parse(&serde_json::json!({"age": {"min": "5"}}))
            .expect("a valid matcher");
        let wheat = state("minecraft:wheat");
        assert!(!predicate.matches(wheat), "wheat starts at age 0");

        let grown = wheat.with(properties::AGE, 7).expect("wheat grows to 7");
        assert!(predicate.matches(grown));
    }

    #[test]
    fn a_property_the_block_does_not_have_never_matches() {
        let predicate =
            StateProperties::parse(&serde_json::json!({"half": "lower"})).expect("a valid matcher");
        assert!(!predicate.matches(state("minecraft:stone")));
    }

    #[test]
    fn every_named_property_has_to_match() {
        let predicate =
            StateProperties::parse(&serde_json::json!({"half": "lower", "open": "true"}))
                .expect("a valid matcher");
        assert!(
            !predicate.matches(state("minecraft:oak_door")),
            "a shut door"
        );
    }
}
