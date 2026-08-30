//! Blocks, as a worldgen definition names them, and the ways it picks one.

use crate::value::{weighted, IntProvider};
use ferrumc_datapack::Identifier;
use ferrumc_world::block_state::BlockId;
use ferrumc_world::block_state_id::BlockStateId;
use serde_json::Value;

/// Reads `{"Name": "minecraft:stone", "Properties": {"axis": "y"}}`.
///
/// The properties are applied one at a time onto the block's default state, so a property the
/// block does not have, or a value it cannot take, makes the whole state unreadable rather than
/// quietly giving the default.
pub fn parse_block_state(value: &Value) -> Option<BlockStateId> {
    let object = value.as_object()?;
    let block = BlockId::from_name(object.get("Name")?.as_str()?)?;
    let mut state = block.default_state();
    if let Some(properties) = object.get("Properties").and_then(Value::as_object) {
        for (name, wanted) in properties {
            let property = block.properties().find(|p| p.name() == name)?;
            state = state.with_raw(property, wanted.as_str()?)?;
        }
    }
    Some(state)
}

/// What a feature checks a block against before replacing it.
#[derive(Clone, Debug)]
pub enum RuleTest {
    /// Always.
    AlwaysTrue,
    /// This block, and nothing else.
    BlockMatch(BlockId),
    /// This state, properties and all.
    BlockStateMatch(BlockStateId),
    /// Anything in this tag.
    TagMatch(Identifier),
    /// One of these blocks, with a chance of passing anyway.
    RandomBlockMatch { block: BlockId, probability: f32 },
    RandomBlockStateMatch {
        state: BlockStateId,
        probability: f32,
    },
}

impl RuleTest {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let kind = object
            .get("predicate_type")
            .or_else(|| object.get("type"))?;
        let kind = kind.as_str()?;
        let block = || {
            object
                .get("block")
                .and_then(Value::as_str)
                .and_then(BlockId::from_name)
        };
        let state = || object.get("block_state").and_then(parse_block_state);
        let probability = || {
            object
                .get("probability")
                .and_then(Value::as_f64)
                .map(|p| p as f32)
        };
        Some(match kind.strip_prefix("minecraft:").unwrap_or(kind) {
            "always_true" => Self::AlwaysTrue,
            "block_match" => Self::BlockMatch(block()?),
            "blockstate_match" => Self::BlockStateMatch(state()?),
            "tag_match" => Self::TagMatch(Identifier::parse(object.get("tag")?.as_str()?).ok()?),
            "random_block_match" => Self::RandomBlockMatch {
                block: block()?,
                probability: probability()?,
            },
            "random_blockstate_match" => Self::RandomBlockStateMatch {
                state: state()?,
                probability: probability()?,
            },
            _ => return None,
        })
    }
}

/// How a feature chooses which block to put down.
#[derive(Clone, Debug)]
pub enum BlockStateProvider {
    /// The same block every time.
    Simple(BlockStateId),
    /// One of several, each as likely as its weight.
    Weighted(Vec<(BlockStateId, i32)>),
    /// The first rule whose question about the world is answered yes, or the fallback.
    RuleBased {
        fallback: Box<BlockStateProvider>,
        rules: Vec<(crate::predicate::BlockPredicate, BlockStateProvider)>,
    },
    /// A block with one of its properties set at random.
    RandomizedInt {
        source: Box<BlockStateProvider>,
        property: String,
        values: IntProvider,
    },
    /// A block turned to face a random way.
    Rotated(BlockStateId),
    /// Chosen by a noise field, which is what makes a patch of a thing look like a patch.
    Noise {
        seed: i64,
        scale: f32,
        states: Vec<BlockStateId>,
    },
    /// The same, with a second field deciding how many kinds appear at all.
    DualNoise {
        seed: i64,
        scale: f32,
        slow_scale: f32,
        variety: (i32, i32),
        states: Vec<BlockStateId>,
    },
    /// One set above a threshold and another below it.
    NoiseThreshold {
        seed: i64,
        scale: f32,
        threshold: f32,
        high_chance: f32,
        default_state: BlockStateId,
        low_states: Vec<BlockStateId>,
        high_states: Vec<BlockStateId>,
    },
}

impl BlockStateProvider {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let kind = object.get("type")?.as_str()?;
        let states = |name: &str| {
            object
                .get(name)?
                .as_array()?
                .iter()
                .map(parse_block_state)
                .collect::<Option<Vec<_>>>()
        };
        let float = |name: &str| object.get(name).and_then(Value::as_f64).map(|v| v as f32);
        let seed = || object.get("seed").and_then(Value::as_i64);
        Some(match kind.strip_prefix("minecraft:").unwrap_or(kind) {
            "simple_state_provider" => Self::Simple(parse_block_state(object.get("state")?)?),
            "weighted_state_provider" => {
                Self::Weighted(weighted(object.get("entries")?, parse_block_state)?)
            }
            "rule_based_state_provider" => Self::RuleBased {
                fallback: Box::new(match object.get("fallback") {
                    Some(fallback) => Self::parse(fallback)?,
                    // A rule set with nothing to fall back on leaves the block alone, which is
                    // what an air state means here.
                    None => Self::Simple(BlockId::from_name("minecraft:air")?.default_state()),
                }),
                rules: object
                    .get("rules")?
                    .as_array()?
                    .iter()
                    .map(|rule| {
                        Some((
                            crate::predicate::BlockPredicate::parse(rule.get("if_true")?)?,
                            Self::parse(rule.get("then")?)?,
                        ))
                    })
                    .collect::<Option<_>>()?,
            },
            "randomized_int_state_provider" => Self::RandomizedInt {
                source: Box::new(Self::parse(object.get("source")?)?),
                property: object.get("property")?.as_str()?.to_owned(),
                values: IntProvider::parse(object.get("values")?)?,
            },
            "rotated_block_provider" => Self::Rotated(parse_block_state(object.get("state")?)?),
            "noise_provider" => Self::Noise {
                seed: seed()?,
                scale: float("scale")?,
                states: states("states")?,
            },
            "dual_noise_provider" => Self::DualNoise {
                seed: seed()?,
                scale: float("scale")?,
                slow_scale: float("slow_scale")?,
                variety: inclusive_range(object.get("variety")?)?,
                states: states("states")?,
            },
            "noise_threshold_provider" => Self::NoiseThreshold {
                seed: seed()?,
                scale: float("scale")?,
                threshold: float("threshold")?,
                high_chance: float("high_chance")?,
                default_state: parse_block_state(object.get("default_state")?)?,
                low_states: states("low_states")?,
                high_states: states("high_states")?,
            },
            _ => return None,
        })
    }
}

/// A range written either as a pair or as an object, both of which the game accepts.
pub fn inclusive_range(value: &Value) -> Option<(i32, i32)> {
    if let Some(pair) = value.as_array() {
        return Some((
            pair.first()?.as_i64()? as i32,
            pair.get(1)?.as_i64()? as i32,
        ));
    }
    Some((
        value.get("min_inclusive")?.as_i64()? as i32,
        value.get("max_inclusive")?.as_i64()? as i32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_state_takes_its_properties() {
        let plain = parse_block_state(&serde_json::json!({"Name": "minecraft:stone"}))
            .expect("stone is a block");
        assert_eq!(
            plain,
            BlockId::from_name("minecraft:stone")
                .expect("stone")
                .default_state()
        );

        let facing = parse_block_state(&serde_json::json!({
            "Name": "minecraft:oak_log", "Properties": {"axis": "x"}
        }))
        .expect("an oak log on its side");
        assert_ne!(
            facing,
            BlockId::from_name("minecraft:oak_log")
                .expect("oak logs exist")
                .default_state()
        );
    }

    /// A property the block does not have makes the state unreadable rather than silently giving
    /// the default, which would put the wrong block in the world.
    #[test]
    fn a_property_that_does_not_belong_is_refused() {
        assert!(parse_block_state(&serde_json::json!({
            "Name": "minecraft:stone", "Properties": {"axis": "x"}
        }))
        .is_none());
        assert!(parse_block_state(&serde_json::json!({
            "Name": "minecraft:oak_log", "Properties": {"axis": "sideways"}
        }))
        .is_none());
        assert!(parse_block_state(&serde_json::json!({"Name": "mypack:invented"})).is_none());
    }

    #[test]
    fn a_provider_reads_its_shapes() {
        let simple = BlockStateProvider::parse(&serde_json::json!({
            "type": "minecraft:simple_state_provider",
            "state": {"Name": "minecraft:stone"}
        }))
        .expect("a simple provider");
        assert!(matches!(simple, BlockStateProvider::Simple(_)));

        let weighted = BlockStateProvider::parse(&serde_json::json!({
            "type": "minecraft:weighted_state_provider",
            "entries": [
                {"data": {"Name": "minecraft:stone"}, "weight": 3},
                {"data": {"Name": "minecraft:dirt"}, "weight": 1}
            ]
        }))
        .expect("a weighted provider");
        let BlockStateProvider::Weighted(entries) = weighted else {
            panic!("a weighted provider")
        };
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].1, 3);
    }
}
