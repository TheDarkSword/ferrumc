//! A set of registry entries, written as one id, a list of them, or a tag.
//!
//! Vanilla's `HolderSet`: `"minecraft:stone"`, `["minecraft:stone", "minecraft:dirt"]` or
//! `"#minecraft:logs"`. A tag is looked up when the set is asked rather than when it is read, so a
//! reload changes what the set holds without anything being parsed again.

use ferrumc_datapack::tag::TagRegistry;
use ferrumc_datapack::Identifier;
use serde_json::Value;

/// Entries named outright, or a tag naming them.
#[derive(Clone, Debug)]
pub enum HolderSet {
    Direct(Vec<Identifier>),
    Tag(Identifier),
}

impl HolderSet {
    /// Reads one id, a list of ids, or a `#tag`.
    pub fn parse(value: &Value) -> Option<Self> {
        match value {
            Value::String(id) => Some(match id.strip_prefix('#') {
                Some(tag) => Self::Tag(Identifier::parse(tag).ok()?),
                None => Self::Direct(vec![Identifier::parse(id).ok()?]),
            }),
            Value::Array(ids) => Some(Self::Direct(
                ids.iter()
                    .filter_map(|id| Identifier::parse(id.as_str()?).ok())
                    .collect(),
            )),
            _ => None,
        }
    }

    /// Whether the set holds the entry with this id and name.
    ///
    /// Both are needed because the two shapes are answered differently: a list is compared by
    /// name, and a tag by the numeric id it was flattened into.
    #[must_use]
    pub fn contains(&self, tags: &TagRegistry, id: u32, name: &str) -> bool {
        match self {
            Self::Direct(entries) => entries.iter().any(|entry| entry.as_str() == name),
            Self::Tag(tag) => tags.get(tag).is_some_and(|tag| tags.contains(tag, id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_all_three_shapes() {
        let one = HolderSet::parse(&serde_json::json!("minecraft:stone")).expect("an id is a set");
        assert!(matches!(one, HolderSet::Direct(ref ids) if ids.len() == 1));

        let many = HolderSet::parse(&serde_json::json!(["minecraft:stone", "minecraft:dirt"]))
            .expect("a list is a set");
        assert!(matches!(many, HolderSet::Direct(ref ids) if ids.len() == 2));

        let tag = HolderSet::parse(&serde_json::json!("#minecraft:logs")).expect("a tag is a set");
        assert!(matches!(tag, HolderSet::Tag(_)));
    }

    #[test]
    fn a_tag_is_answered_by_the_tags_as_they_stand() {
        let tags = ferrumc_registry::tags::current();
        let blocks = tags.block();
        let logs = HolderSet::parse(&serde_json::json!("#minecraft:logs")).expect("a tag is a set");
        let oak = ferrumc_world::block_state::BlockId::from_name("minecraft:oak_log")
            .expect("oak logs exist");

        assert!(logs.contains(&blocks, u32::from(oak.index()), oak.name()));
        let stone = ferrumc_world::block_state::BlockId::from_name("minecraft:stone")
            .expect("stone exists");
        assert!(!logs.contains(&blocks, u32::from(stone.index()), stone.name()));
    }

    #[test]
    fn a_list_is_answered_by_name() {
        let tags = ferrumc_registry::tags::current();
        let blocks = tags.block();
        let set =
            HolderSet::parse(&serde_json::json!(["minecraft:stone"])).expect("a list is a set");
        assert!(set.contains(&blocks, 0, "minecraft:stone"));
        assert!(!set.contains(&blocks, 0, "minecraft:dirt"));
    }
}
