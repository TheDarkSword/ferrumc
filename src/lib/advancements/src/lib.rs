//! Advancements: the tree of things a player has done.
//!
//! An advancement is a set of named criteria, each a trigger with conditions, plus a rule saying
//! which of them together count as done. A trigger fires from gameplay — an item picked up, a mob
//! killed, a place reached — and every criterion listening for it is offered the event.
//!
//! What a player has done is kept per player and written with the rest of their data, so the tree
//! is where they left it after a restart.

pub mod display;
pub mod layout;
pub mod progress;
pub mod trigger;

use display::DisplayInfo;
use ferrumc_datapack::manager::FileToId;
use ferrumc_datapack::{Identifier, ResourceManager};
use serde_json::Value;
use std::collections::BTreeMap;
use tracing::error;
use trigger::Criterion;

pub use progress::{AdvancementProgress, PlayerAdvancements};
pub use trigger::Trigger;

/// Where a pack keeps its advancements.
pub const DIRECTORY: &str = "advancement";

/// Why an advancement could not be read.
#[derive(Debug, thiserror::Error)]
pub enum AdvancementError {
    #[error("advancement is not an object")]
    NotAnObject,
    #[error("advancement has no criteria")]
    NoCriteria,
    #[error("criterion '{0}' has no trigger")]
    NoTrigger(String),
    #[error("unknown trigger '{0}'")]
    UnknownTrigger(String),
    #[error("requirements name a criterion that is not there: {0}")]
    UnknownRequirement(String),
}

/// Which criteria together count as done: every group has to have one of its criteria granted.
#[derive(Clone, Debug, Default)]
pub struct Requirements(pub Vec<Vec<String>>);

impl Requirements {
    /// Vanilla's default when a file says nothing: all of them, each in a group of its own.
    fn all_of(criteria: &BTreeMap<String, Criterion>) -> Self {
        Self(criteria.keys().map(|name| vec![name.clone()]).collect())
    }

    fn parse(value: &Value) -> Option<Self> {
        Some(Self(
            value
                .as_array()?
                .iter()
                .map(|group| {
                    group
                        .as_array()?
                        .iter()
                        .map(|name| Some(name.as_str()?.to_owned()))
                        .collect::<Option<Vec<_>>>()
                })
                .collect::<Option<_>>()?,
        ))
    }

    /// Whether enough is granted. An advancement with no groups is never done, as vanilla has it.
    #[must_use]
    pub fn met(&self, granted: impl Fn(&str) -> bool) -> bool {
        !self.0.is_empty()
            && self
                .0
                .iter()
                .all(|group| group.iter().any(|name| granted(name)))
    }
}

/// What is given for finishing one.
#[derive(Clone, Debug, Default)]
pub struct Rewards {
    pub experience: i32,
    pub loot: Vec<Identifier>,
    pub recipes: Vec<Identifier>,
    pub function: Option<Identifier>,
}

impl Rewards {
    fn parse(value: &Value) -> Self {
        let list = |name: &str| {
            value
                .get(name)
                .and_then(Value::as_array)
                .map(|ids| {
                    ids.iter()
                        .filter_map(|id| Identifier::parse(id.as_str()?).ok())
                        .collect()
                })
                .unwrap_or_default()
        };
        Self {
            experience: value
                .get("experience")
                .and_then(Value::as_i64)
                .and_then(|xp| i32::try_from(xp).ok())
                .unwrap_or_default(),
            loot: list("loot"),
            recipes: list("recipes"),
            function: value
                .get("function")
                .and_then(Value::as_str)
                .and_then(|id| Identifier::parse(id).ok()),
        }
    }
}

/// One advancement.
#[derive(Clone, Debug)]
pub struct Advancement {
    /// The one above it, absent for a root.
    pub parent: Option<Identifier>,
    /// How it appears on the screen. An advancement with none is invisible, which is what every
    /// recipe unlock is.
    pub display: Option<DisplayInfo>,
    pub rewards: Rewards,
    pub criteria: BTreeMap<String, Criterion>,
    pub requirements: Requirements,
    pub sends_telemetry_event: bool,
}

impl Advancement {
    pub fn parse(value: &Value) -> Result<Self, AdvancementError> {
        let object = value.as_object().ok_or(AdvancementError::NotAnObject)?;
        let criteria_json = object
            .get("criteria")
            .and_then(Value::as_object)
            .filter(|criteria| !criteria.is_empty())
            .ok_or(AdvancementError::NoCriteria)?;

        let mut criteria = BTreeMap::new();
        for (name, criterion) in criteria_json {
            criteria.insert(name.clone(), Criterion::parse(name, criterion)?);
        }

        let requirements = object
            .get("requirements")
            .and_then(Requirements::parse)
            .unwrap_or_else(|| Requirements::all_of(&criteria));
        for group in &requirements.0 {
            for name in group {
                if !criteria.contains_key(name) {
                    return Err(AdvancementError::UnknownRequirement(name.clone()));
                }
            }
        }

        Ok(Self {
            parent: object
                .get("parent")
                .and_then(Value::as_str)
                .and_then(|id| Identifier::parse(id).ok()),
            display: object.get("display").and_then(DisplayInfo::parse),
            rewards: object
                .get("rewards")
                .map(Rewards::parse)
                .unwrap_or_default(),
            criteria,
            requirements,
            sends_telemetry_event: object
                .get("sends_telemetry_event")
                .and_then(Value::as_bool)
                .unwrap_or_default(),
        })
    }

    #[must_use]
    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }
}

/// Every advancement the loaded packs declare, with the tree they form.
#[derive(Debug, Default)]
pub struct Advancements {
    by_name: BTreeMap<String, Advancement>,
    /// Where each visible one sits on the screen, worked out once when they are read.
    positions: BTreeMap<String, (f32, f32)>,
}

impl Advancements {
    /// Reads every advancement in a pack stack and lays out the trees they form.
    #[must_use]
    pub fn load(manager: &ResourceManager) -> Self {
        let mut by_name = BTreeMap::new();
        for (id, resource) in FileToId::json(DIRECTORY).list(manager) {
            match serde_json::from_slice(&resource.data)
                .map_err(|e| e.to_string())
                .and_then(|value: Value| Advancement::parse(&value).map_err(|e| e.to_string()))
            {
                Ok(advancement) => {
                    by_name.insert(id.as_str().to_owned(), advancement);
                }
                Err(e) => error!(
                    "couldn't read advancement {id} from data pack {}: {e}",
                    resource.source
                ),
            }
        }
        let positions = layout::lay_out(&by_name);
        Self { by_name, positions }
    }

    #[must_use]
    pub fn get(&self, name: &Identifier) -> Option<&Advancement> {
        self.by_name.get(name.as_str())
    }

    #[must_use]
    pub fn get_by_name(&self, name: &str) -> Option<&Advancement> {
        self.by_name.get(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Advancement)> {
        self.by_name
            .iter()
            .map(|(name, advancement)| (&**name, advancement))
    }

    /// Where a visible advancement sits on its tree.
    #[must_use]
    pub fn position(&self, name: &str) -> (f32, f32) {
        self.positions.get(name).copied().unwrap_or((0.0, 0.0))
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

#[cfg(test)]
mod tests;
