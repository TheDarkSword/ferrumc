//! The conditions themselves, and the registry of the ones a datapack names.
//!
//! Vanilla's `LootItemConditions`. Every one of them is a `condition` field naming a type and
//! whatever that type reads; they nest freely through `all_of`, `any_of` and `inverted`.

use crate::block::BlockPredicate;
use crate::context::LootContext;
use crate::holders::HolderSet;
use crate::item::ItemPredicate;
use crate::location::LocationPredicate;
use crate::number::{IntRange, NumberProvider};
use ferrumc_datapack::manager::FileToId;
use ferrumc_datapack::{Identifier, ResourceManager};
use serde_json::Value;
use std::collections::BTreeMap;
use tracing::{error, warn};

/// Why a condition could not be read.
#[derive(Debug, thiserror::Error)]
pub enum ConditionError {
    #[error("condition is not an object")]
    NotAnObject,
    #[error("condition has no type")]
    NoType,
    #[error("unknown condition type '{0}'")]
    UnknownType(String),
    #[error("condition '{kind}' is missing or malformed: {field}")]
    BadField { kind: String, field: String },
}

/// A condition, as vanilla writes them.
#[derive(Clone, Debug)]
pub enum Condition {
    /// Every term has to hold.
    AllOf(Vec<Condition>),
    /// Any one term holding is enough.
    AnyOf(Vec<Condition>),
    Inverted(Box<Condition>),
    /// A roll against the given chance.
    RandomChance(NumberProvider),
    /// Whether the block survived being blown up, which is likelier the smaller the blast.
    SurvivesExplosion,
    /// Whether the block being broken is this one, in this state.
    BlockStateProperty(BlockPredicate),
    /// Whether the tool used is this one.
    MatchTool(Option<ItemPredicate>),
    /// Whether the place this is happening is like this.
    LocationCheck {
        predicate: Option<LocationPredicate>,
        offset: (i32, i32, i32),
    },
    /// Whether a player struck the last blow.
    KilledByPlayer,
    /// A roll against the chance for the tool's level of an enchantment. Without enchantments the
    /// level is nought, which is the first chance in the list.
    TableBonus {
        chances: Vec<f32>,
    },
    /// What the sky is doing.
    WeatherCheck {
        raining: Option<bool>,
        thundering: Option<bool>,
    },
    /// Whether the world clock reads within a range.
    TimeCheck {
        period: Option<i64>,
        value: IntRange,
    },
    /// Whether a number lands in a range.
    ValueCheck {
        value: NumberProvider,
        range: IntRange,
    },
    /// Whether the enchantment that caused this is active.
    EnchantmentActiveCheck(bool),
    /// Another predicate, named.
    Reference(Identifier),
    /// A condition whose source does not exist yet. Never holds, and says so once.
    Unsupported(&'static str),
}

impl Condition {
    /// Reads a condition, or a bare list of them meaning all of them.
    pub fn parse(value: &Value) -> Result<Self, ConditionError> {
        if let Some(terms) = value.as_array() {
            // A bare list is an `all_of`, which is the inline form vanilla accepts anywhere a
            // condition is expected.
            return Ok(Self::AllOf(
                terms.iter().map(Self::parse).collect::<Result<_, _>>()?,
            ));
        }
        let object = value.as_object().ok_or(ConditionError::NotAnObject)?;
        let kind = object
            .get("condition")
            .and_then(Value::as_str)
            .ok_or(ConditionError::NoType)?;
        let bare = kind.strip_prefix("minecraft:").unwrap_or(kind);
        let bad = |field: &str| ConditionError::BadField {
            kind: kind.to_owned(),
            field: field.to_owned(),
        };
        let terms = || -> Result<Vec<Self>, ConditionError> {
            object
                .get("terms")
                .and_then(Value::as_array)
                .ok_or_else(|| bad("terms"))?
                .iter()
                .map(Self::parse)
                .collect()
        };

        Ok(match bare {
            "all_of" => Self::AllOf(terms()?),
            "any_of" => Self::AnyOf(terms()?),
            "inverted" => Self::Inverted(Box::new(Self::parse(
                object.get("term").ok_or_else(|| bad("term"))?,
            )?)),
            "random_chance" => Self::RandomChance(
                object
                    .get("chance")
                    .and_then(NumberProvider::parse)
                    .ok_or_else(|| bad("chance"))?,
            ),
            "survives_explosion" => Self::SurvivesExplosion,
            "block_state_property" => {
                // The block is a bare id here rather than a set, and the state field is called
                // `properties` rather than `state`, so it is read into the same predicate by hand.
                let block = object
                    .get("block")
                    .and_then(Value::as_str)
                    .ok_or_else(|| bad("block"))?;
                Self::BlockStateProperty(BlockPredicate::of(
                    HolderSet::parse(&Value::String(block.to_owned())),
                    object
                        .get("properties")
                        .and_then(crate::state::StateProperties::parse),
                ))
            }
            "match_tool" => Self::MatchTool(object.get("predicate").and_then(ItemPredicate::parse)),
            "location_check" => Self::LocationCheck {
                predicate: object.get("predicate").and_then(LocationPredicate::parse),
                offset: (
                    offset(object.get("offsetX")),
                    offset(object.get("offsetY")),
                    offset(object.get("offsetZ")),
                ),
            },
            "killed_by_player" => Self::KilledByPlayer,
            "table_bonus" => Self::TableBonus {
                chances: object
                    .get("chances")
                    .and_then(Value::as_array)
                    .ok_or_else(|| bad("chances"))?
                    .iter()
                    .filter_map(|chance| Some(chance.as_f64()? as f32))
                    .collect(),
            },
            "weather_check" => Self::WeatherCheck {
                raining: object.get("raining").and_then(Value::as_bool),
                thundering: object.get("thundering").and_then(Value::as_bool),
            },
            "time_check" => Self::TimeCheck {
                period: object.get("period").and_then(Value::as_i64),
                value: object
                    .get("value")
                    .and_then(IntRange::parse)
                    .ok_or_else(|| bad("value"))?,
            },
            "value_check" => Self::ValueCheck {
                value: object
                    .get("value")
                    .and_then(NumberProvider::parse)
                    .ok_or_else(|| bad("value"))?,
                range: object
                    .get("range")
                    .and_then(IntRange::parse)
                    .ok_or_else(|| bad("range"))?,
            },
            "enchantment_active_check" => Self::EnchantmentActiveCheck(
                object
                    .get("active")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| bad("active"))?,
            ),
            "reference" => Self::Reference(
                object
                    .get("name")
                    .and_then(Value::as_str)
                    .and_then(|name| Identifier::parse(name).ok())
                    .ok_or_else(|| bad("name"))?,
            ),
            // These need an entity, a damage source or an enchantment to ask about, and none of
            // the three exists yet. They are read so a file carrying one still loads.
            "entity_properties" => Self::Unsupported("entity_properties"),
            "entity_scores" => Self::Unsupported("entity_scores"),
            "damage_source_properties" => Self::Unsupported("damage_source_properties"),
            "random_chance_with_enchanted_bonus" => {
                Self::Unsupported("random_chance_with_enchanted_bonus")
            }
            "environment_attribute_check" => Self::Unsupported("environment_attribute_check"),
            _ => return Err(ConditionError::UnknownType(kind.to_owned())),
        })
    }

    /// Whether the condition holds.
    pub fn test(&self, context: &mut LootContext) -> bool {
        match self {
            Self::AllOf(terms) => terms.iter().all(|term| term.test(context)),
            Self::AnyOf(terms) => terms.iter().any(|term| term.test(context)),
            Self::Inverted(term) => !term.test(context),
            Self::RandomChance(chance) => {
                let chance = chance.float(context);
                context.next_float() < chance
            }
            // No explosion means nothing was blown up, so everything survives.
            Self::SurvivesExplosion => match context.params.explosion_radius {
                Some(radius) => context.next_float() <= 1.0 / radius,
                None => true,
            },
            Self::BlockStateProperty(predicate) => {
                let tags = context.tags.block();
                context
                    .params
                    .block_state
                    .is_some_and(|state| predicate.matches_state(&tags, state))
            }
            Self::MatchTool(predicate) => {
                let tags = context.tags.item();
                context.params.tool.is_some_and(|tool| {
                    predicate
                        .as_ref()
                        .is_none_or(|predicate| predicate.matches(&tags, tool))
                })
            }
            Self::LocationCheck { predicate, offset } => {
                let Some(origin) = context.params.origin else {
                    return false;
                };
                let Some(world) = context.world else {
                    return false;
                };
                let tags = context.tags.block();
                predicate.as_ref().is_none_or(|predicate| {
                    predicate.matches(
                        world,
                        &tags,
                        crate::context::Origin {
                            x: origin.x + f64::from(offset.0),
                            y: origin.y + f64::from(offset.1),
                            z: origin.z + f64::from(offset.2),
                        },
                    )
                })
            }
            Self::KilledByPlayer => context.params.killed_by_player,
            // Without enchantments the level is nought, which is the first chance in the list.
            Self::TableBonus { chances } => {
                let chance = chances.first().copied().unwrap_or_default();
                context.next_float() < chance
            }
            Self::WeatherCheck {
                raining,
                thundering,
            } => {
                let Some(world) = context.world else {
                    return false;
                };
                raining.is_none_or(|expected| expected == world.is_raining())
                    && thundering.is_none_or(|expected| expected == world.is_thundering())
            }
            Self::TimeCheck { period, value } => {
                let Some(world) = context.world else {
                    return false;
                };
                let mut time = world.time();
                if let Some(period) = period {
                    if *period != 0 {
                        time %= period;
                    }
                }
                let time = i32::try_from(time).unwrap_or(i32::MAX);
                value.matches(context, time)
            }
            Self::ValueCheck { value, range } => {
                let value = value.int(context);
                range.matches(context, value)
            }
            Self::EnchantmentActiveCheck(expected) => {
                context.params.enchantment_active == Some(*expected)
            }
            Self::Reference(name) => {
                let Some(predicates) = context.predicates else {
                    warn!("tried using condition {name} with no predicates loaded");
                    return false;
                };
                let Some(condition) = predicates.get(name).cloned() else {
                    warn!("tried using unknown condition table called {name}");
                    return false;
                };
                if context.visiting.iter().any(|seen| seen == name.as_str()) {
                    warn!("detected infinite loop in loot tables");
                    return false;
                }
                context.visiting.push(name.as_str().to_owned());
                let held = condition.test(context);
                context.visiting.pop();
                held
            }
            Self::Unsupported(kind) => {
                warn!("condition {kind} is not supported yet, treating it as not holding");
                false
            }
        }
    }
}

fn offset(value: Option<&Value>) -> i32 {
    value
        .and_then(Value::as_i64)
        .and_then(|v| i32::try_from(v).ok())
        .unwrap_or_default()
}

/// Where a pack keeps the predicates a `reference` can name.
pub const DIRECTORY: &str = "predicate";

/// The predicates a datapack declares, by name.
#[derive(Debug, Default)]
pub struct Predicates {
    by_name: BTreeMap<String, Condition>,
}

impl Predicates {
    /// Reads every predicate file in a pack stack.
    #[must_use]
    pub fn load(manager: &ResourceManager) -> Self {
        let mut by_name = BTreeMap::new();
        for (id, resource) in FileToId::json(DIRECTORY).list(manager) {
            match serde_json::from_slice(&resource.data)
                .map_err(|e| e.to_string())
                .and_then(|value: Value| Condition::parse(&value).map_err(|e| e.to_string()))
            {
                Ok(condition) => {
                    by_name.insert(id.as_str().to_owned(), condition);
                }
                Err(e) => error!(
                    "couldn't read predicate {id} from data pack {}: {e}",
                    resource.source
                ),
            }
        }
        Self { by_name }
    }

    #[must_use]
    pub fn get(&self, name: &Identifier) -> Option<&Condition> {
        self.by_name.get(name.as_str())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}
