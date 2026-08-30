//! What a feature asks about a block before it does anything there.
//!
//! Vanilla's worldgen `BlockPredicate`, which is a different thing from the one loot tables and
//! advancements share: this one looks at a position in the world being generated, and every kind
//! of it may be asked about a neighbour rather than the block itself.

use crate::state::parse_block_state;
use ferrumc_datapack::Identifier;
use ferrumc_world::block_state::{BlockId, Direction};
use ferrumc_world::block_state_id::BlockStateId;
use serde_json::Value;

/// Which way from the block being placed to look.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Offset {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Offset {
    fn parse(value: Option<&Value>) -> Self {
        let Some(offset) = value.and_then(Value::as_array) else {
            return Self::default();
        };
        let at = |index: usize| {
            offset
                .get(index)
                .and_then(Value::as_i64)
                .map(|v| v as i32)
                .unwrap_or_default()
        };
        Self {
            x: at(0),
            y: at(1),
            z: at(2),
        }
    }
}

/// A question about one block of the world being generated.
#[derive(Clone, Debug)]
pub enum BlockPredicate {
    /// Always.
    True,
    /// One of these blocks is there.
    MatchingBlocks {
        offset: Offset,
        blocks: Vec<BlockId>,
    },
    /// Something in this tag is there.
    MatchingBlockTag {
        offset: Offset,
        tag: Identifier,
    },
    /// One of these fluids is there.
    MatchingFluids {
        offset: Offset,
        fluids: Vec<Identifier>,
    },
    /// The biome there is one of these.
    MatchingBiomes {
        offset: Offset,
        biomes: Vec<Identifier>,
    },
    /// The block there has a face solid enough to hang something off.
    HasSturdyFace {
        offset: Offset,
        direction: Direction,
    },
    /// The block there is solid.
    Solid {
        offset: Offset,
    },
    /// The block there can be built over.
    Replaceable {
        offset: Offset,
    },
    /// This state would stay where it was put.
    WouldSurvive {
        offset: Offset,
        state: BlockStateId,
    },
    /// The place is inside the world at all.
    InsideWorldBounds {
        offset: Offset,
    },
    /// Nothing is in the way between here and there.
    Unobstructed {
        offset: Offset,
    },
    AllOf(Vec<BlockPredicate>),
    AnyOf(Vec<BlockPredicate>),
    Not(Box<BlockPredicate>),
}

impl BlockPredicate {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let kind = object.get("type")?.as_str()?;
        let offset = Offset::parse(object.get("offset"));
        let ids = |name: &str| -> Option<Vec<Identifier>> {
            match object.get(name)? {
                Value::String(one) => Some(vec![Identifier::parse(one).ok()?]),
                Value::Array(many) => many
                    .iter()
                    .map(|id| Identifier::parse(id.as_str()?).ok())
                    .collect(),
                _ => None,
            }
        };
        let terms = |name: &str| {
            object
                .get(name)?
                .as_array()?
                .iter()
                .map(Self::parse)
                .collect::<Option<Vec<_>>>()
        };
        Some(match kind.strip_prefix("minecraft:").unwrap_or(kind) {
            "true" => Self::True,
            "matching_blocks" => Self::MatchingBlocks {
                offset,
                // A list of blocks by name; a tag is a kind of its own here.
                blocks: ids("blocks")?
                    .iter()
                    .map(|id| BlockId::from_name(id.as_str()))
                    .collect::<Option<_>>()?,
            },
            "matching_block_tag" => Self::MatchingBlockTag {
                offset,
                tag: Identifier::parse(object.get("tag")?.as_str()?).ok()?,
            },
            "matching_fluids" => Self::MatchingFluids {
                offset,
                fluids: ids("fluids")?,
            },
            "matching_biomes" => Self::MatchingBiomes {
                offset,
                biomes: ids("biomes")?,
            },
            "has_sturdy_face" => Self::HasSturdyFace {
                offset,
                direction: direction(object.get("direction")?.as_str()?)?,
            },
            "solid" => Self::Solid { offset },
            "replaceable" => Self::Replaceable { offset },
            "would_survive" => Self::WouldSurvive {
                offset,
                state: parse_block_state(object.get("state")?)?,
            },
            "inside_world_bounds" => Self::InsideWorldBounds { offset },
            "unobstructed" => Self::Unobstructed { offset },
            "all_of" => Self::AllOf(terms("predicates")?),
            "any_of" => Self::AnyOf(terms("predicates")?),
            "not" => Self::Not(Box::new(Self::parse(object.get("predicate")?)?)),
            _ => return None,
        })
    }
}

fn direction(name: &str) -> Option<Direction> {
    Some(match name {
        "down" => Direction::Down,
        "up" => Direction::Up,
        "north" => Direction::North,
        "south" => Direction::South,
        "west" => Direction::West,
        "east" => Direction::East,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_predicate_may_ask_about_a_neighbour() {
        let below = BlockPredicate::parse(&serde_json::json!({
            "type": "minecraft:matching_blocks",
            "offset": [0, -1, 0],
            "blocks": "minecraft:grass_block"
        }))
        .expect("a valid predicate");
        let BlockPredicate::MatchingBlocks { offset, blocks } = below else {
            panic!("matching blocks")
        };
        assert_eq!(offset, Offset { x: 0, y: -1, z: 0 });
        assert_eq!(blocks.len(), 1);
    }

    /// With no offset the question is about the block itself.
    #[test]
    fn no_offset_means_here() {
        let here =
            BlockPredicate::parse(&serde_json::json!({"type": "minecraft:solid"})).expect("solid");
        assert!(matches!(
            here,
            BlockPredicate::Solid {
                offset: Offset { x: 0, y: 0, z: 0 }
            }
        ));
    }

    #[test]
    fn predicates_nest() {
        let nested = BlockPredicate::parse(&serde_json::json!({
            "type": "minecraft:not",
            "predicate": {
                "type": "minecraft:any_of",
                "predicates": [
                    {"type": "minecraft:solid"},
                    {"type": "minecraft:matching_block_tag", "tag": "minecraft:logs"}
                ]
            }
        }))
        .expect("a valid predicate");
        let BlockPredicate::Not(inner) = nested else {
            panic!("a not")
        };
        assert!(matches!(*inner, BlockPredicate::AnyOf(ref terms) if terms.len() == 2));
    }
}
