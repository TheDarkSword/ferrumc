//! What a loot table does to a stack after it has been picked.
//!
//! Vanilla's `LootItemFunction`. Most of them set a component — an enchantment, a name, a potion —
//! and a stack carries none yet, so those are read and leave the stack alone. The ones that touch
//! the count all work.

use crate::stack::ItemStack;
use ferrumc_datapack::Identifier;
use ferrumc_predicates::condition::Condition;
use ferrumc_predicates::number::{IntRange, NumberProvider};
use ferrumc_predicates::{ItemPredicate, LootContext};
use serde_json::Value;
use tracing::warn;

/// Why a function could not be read.
#[derive(Debug, thiserror::Error)]
pub enum FunctionError {
    #[error("function is not an object")]
    NotAnObject,
    #[error("function has no type")]
    NoType,
    #[error("unknown function type '{0}'")]
    UnknownType(String),
    #[error("function '{kind}' is missing or malformed: {field}")]
    BadField { kind: String, field: String },
    #[error(transparent)]
    Condition(#[from] ferrumc_predicates::condition::ConditionError),
}

/// How `apply_bonus` turns a tool's enchantment level into more of something.
#[derive(Clone, Debug)]
pub enum BonusFormula {
    /// A roll per level, plus a few free ones.
    BinomialWithBonusCount { extra_rounds: i32, probability: f32 },
    /// What ore drops: a multiplier of one to level plus one, weighted towards one.
    OreDrops,
    /// A flat roll of nought to multiplier times level.
    UniformBonusCount { multiplier: i32 },
}

impl BonusFormula {
    fn apply(&self, context: &mut LootContext, count: i32, level: i32) -> i32 {
        match self {
            Self::BinomialWithBonusCount {
                extra_rounds,
                probability,
            } => {
                let mut count = count;
                for _ in 0..(level + extra_rounds) {
                    if context.next_float() < *probability {
                        count += 1;
                    }
                }
                count
            }
            // Fortune on ore: a roll of minus one to level, floored at nought, and the drop is
            // multiplied by one more than that. So fortune one is an even chance of doubling.
            Self::OreDrops => {
                if level > 0 {
                    let bonus = context.next_int(0, level + 1) - 1;
                    count * (bonus.max(0) + 1)
                } else {
                    count
                }
            }
            Self::UniformBonusCount { multiplier } => {
                count + context.next_int(0, multiplier * level)
            }
        }
    }
}

/// A function, with the conditions that gate it.
#[derive(Clone, Debug)]
pub struct Function {
    pub conditions: Vec<Condition>,
    pub kind: FunctionKind,
}

#[derive(Clone, Debug)]
pub enum FunctionKind {
    SetCount {
        count: NumberProvider,
        /// Whether the count is added to what is there rather than replacing it.
        add: bool,
    },
    /// A blast destroys some of what a block would have dropped.
    ExplosionDecay,
    LimitCount(IntRange),
    ApplyBonus {
        formula: BonusFormula,
    },
    /// Runs its own functions in order.
    Sequence(Vec<Function>),
    /// Runs a function only on the stacks that match.
    Filtered {
        item_filter: ItemPredicate,
        modifier: Box<Function>,
    },
    /// A function whose effect is on something a stack does not carry yet. Read, and leaves the
    /// stack as it found it.
    Untouched(&'static str),
}

impl Function {
    pub fn parse(value: &Value) -> Result<Self, FunctionError> {
        let object = value.as_object().ok_or(FunctionError::NotAnObject)?;
        let kind = object
            .get("function")
            .and_then(Value::as_str)
            .ok_or(FunctionError::NoType)?;
        let bare = kind.strip_prefix("minecraft:").unwrap_or(kind);
        let bad = |field: &str| FunctionError::BadField {
            kind: kind.to_owned(),
            field: field.to_owned(),
        };
        let conditions = crate::conditions(object.get("conditions"))?;

        let kind = match bare {
            "set_count" => FunctionKind::SetCount {
                count: object
                    .get("count")
                    .and_then(NumberProvider::parse)
                    .ok_or_else(|| bad("count"))?,
                add: object
                    .get("add")
                    .and_then(Value::as_bool)
                    .unwrap_or_default(),
            },
            "explosion_decay" => FunctionKind::ExplosionDecay,
            "limit_count" => FunctionKind::LimitCount(
                object
                    .get("limit")
                    .and_then(IntRange::parse)
                    .ok_or_else(|| bad("limit"))?,
            ),
            "apply_bonus" => FunctionKind::ApplyBonus {
                formula: bonus_formula(object).ok_or_else(|| bad("formula"))?,
            },
            "sequence" => FunctionKind::Sequence(
                object
                    .get("functions")
                    .and_then(Value::as_array)
                    .ok_or_else(|| bad("functions"))?
                    .iter()
                    .map(Self::parse)
                    .collect::<Result<_, _>>()?,
            ),
            "filtered" => FunctionKind::Filtered {
                item_filter: object
                    .get("item_filter")
                    .and_then(ItemPredicate::parse)
                    .ok_or_else(|| bad("item_filter"))?,
                modifier: Box::new(Self::parse(
                    object.get("modifier").ok_or_else(|| bad("modifier"))?,
                )?),
            },
            // Everything else changes a component, needs a recipe or a map, or names another
            // function. All of them are read so a table carrying one still loads.
            other if KNOWN.contains(&other) => FunctionKind::Untouched(
                KNOWN
                    .iter()
                    .find(|known| **known == other)
                    .copied()
                    .unwrap_or("unknown"),
            ),
            _ => return Err(FunctionError::UnknownType(kind.to_owned())),
        };
        Ok(Self { conditions, kind })
    }

    /// Runs the function, if its conditions hold.
    pub fn apply(&self, context: &mut LootContext, stack: ItemStack) -> ItemStack {
        if !self
            .conditions
            .iter()
            .all(|condition| condition.test(context))
        {
            return stack;
        }
        let mut stack = stack;
        match &self.kind {
            FunctionKind::SetCount { count, add } => {
                let base = if *add { stack.count } else { 0 };
                stack.count = base + count.int(context);
            }
            // Each item of the stack takes its own chance of surviving the blast.
            FunctionKind::ExplosionDecay => {
                if let Some(radius) = context.params.explosion_radius {
                    let probability = 1.0 / radius;
                    let survived = (0..stack.count)
                        .filter(|_| context.next_float() <= probability)
                        .count();
                    stack.count = i32::try_from(survived).unwrap_or(stack.count);
                }
            }
            FunctionKind::LimitCount(limit) => {
                stack.count = limit.clamp(context, stack.count);
            }
            // Without an enchantment on the tool the level is nought, which every formula reads
            // as no bonus at all.
            FunctionKind::ApplyBonus { formula } => {
                if context.params.tool.is_some() {
                    let level = 0;
                    stack.count = formula.apply(context, stack.count, level);
                }
            }
            FunctionKind::Sequence(functions) => {
                for function in functions {
                    stack = function.apply(context, stack);
                }
            }
            FunctionKind::Filtered {
                item_filter,
                modifier,
            } => {
                let tags = context.tags.item();
                let matches = item_filter.matches(
                    &tags,
                    ferrumc_predicates::context::ItemRef {
                        id: stack.item,
                        count: stack.count,
                    },
                );
                if matches {
                    stack = modifier.apply(context, stack);
                }
            }
            FunctionKind::Untouched(kind) => {
                warn!("loot function {kind} is not supported yet, leaving the stack alone");
            }
        }
        stack
    }
}

fn bonus_formula(object: &serde_json::Map<String, Value>) -> Option<BonusFormula> {
    let formula = object.get("formula")?.as_str()?;
    let parameters = object.get("parameters");
    let parameter = |name: &str| parameters.and_then(|p| p.get(name));
    Some(
        match formula.strip_prefix("minecraft:").unwrap_or(formula) {
            "binomial_with_bonus_count" => BonusFormula::BinomialWithBonusCount {
                extra_rounds: i32::try_from(parameter("extra")?.as_i64()?).ok()?,
                probability: parameter("probability")?.as_f64()? as f32,
            },
            "ore_drops" => BonusFormula::OreDrops,
            "uniform_bonus_count" => BonusFormula::UniformBonusCount {
                multiplier: parameter("bonusMultiplier")
                    .and_then(Value::as_i64)
                    .and_then(|v| i32::try_from(v).ok())
                    .unwrap_or(1),
            },
            _ => return None,
        },
    )
}

/// Every function the game has, so one that is not run is still told apart from one that does not
/// exist. What each of them waits on is in the datapack documentation.
const KNOWN: &[&str] = &[
    "set_item",
    "enchant_with_levels",
    "enchant_randomly",
    "set_enchantments",
    "set_custom_data",
    "set_components",
    "furnace_smelt",
    "enchanted_count_increase",
    "set_damage",
    "set_attributes",
    "set_name",
    "exploration_map",
    "set_stew_effect",
    "copy_name",
    "set_contents",
    "modify_contents",
    "set_loot_table",
    "set_lore",
    "fill_player_head",
    "copy_custom_data",
    "copy_state",
    "set_banner_pattern",
    "set_potion",
    "set_random_dyes",
    "set_random_potion",
    "set_instrument",
    "reference",
    "copy_components",
    "set_fireworks",
    "set_firework_explosion",
    "set_book_cover",
    "set_written_book_pages",
    "set_writable_book_pages",
    "toggle_tooltips",
    "set_ominous_bottle_amplifier",
    "set_custom_model_data",
    "discard",
];

/// A name for an id, for the tests and for logs.
#[must_use]
pub fn item_id(name: &str) -> Option<i32> {
    ferrumc_registry::lookup_item_protocol_id(name)
}

/// Reads the id a `name` field names.
pub(crate) fn named_item(value: Option<&Value>) -> Option<i32> {
    item_id(&Identifier::parse(value?.as_str()?).ok()?.to_string())
}
