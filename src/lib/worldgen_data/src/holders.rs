//! A set of registry entries, written as one id, a list of them, or a tag.
//!
//! Vanilla's `HolderSet`, which the worldgen data uses for biomes, carvers and blocks alike. A
//! single entry is written bare rather than as a list of one, which is easy to miss: five of the
//! game's own biomes name one carver and would not read as a list.

use ferrumc_datapack::Identifier;
use serde_json::Value;

#[derive(Clone, Debug)]
pub enum IdSet {
    Direct(Vec<Identifier>),
    Tag(Identifier),
}

impl IdSet {
    pub fn parse(value: &Value) -> Option<Self> {
        match value {
            Value::String(one) => Some(match one.strip_prefix('#') {
                Some(tag) => Self::Tag(Identifier::parse(tag).ok()?),
                None => Self::Direct(vec![Identifier::parse(one).ok()?]),
            }),
            Value::Array(many) => Some(Self::Direct(
                many.iter()
                    .map(|id| Identifier::parse(id.as_str()?).ok())
                    .collect::<Option<_>>()?,
            )),
            _ => None,
        }
    }

    /// The entries named outright, empty where it is a tag.
    #[must_use]
    pub fn named(&self) -> &[Identifier] {
        match self {
            Self::Direct(ids) => ids,
            Self::Tag(_) => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_entry_is_written_bare() {
        let one = IdSet::parse(&serde_json::json!("minecraft:nether_cave")).expect("one id");
        assert_eq!(one.named().len(), 1);

        let many = IdSet::parse(&serde_json::json!(["minecraft:cave", "minecraft:canyon"]))
            .expect("a list");
        assert_eq!(many.named().len(), 2);

        let tag = IdSet::parse(&serde_json::json!("#minecraft:is_overworld")).expect("a tag");
        assert!(matches!(tag, IdSet::Tag(_)));
        assert!(tag.named().is_empty());
    }
}
