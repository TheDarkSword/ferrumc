//! Loot tables: where every drop in the game comes from.
//!
//! A table is a list of pools. A pool rolls a count, and each roll draws one entry from the ones
//! in the running, weighted. An entry produces item stacks, and functions modify them on the way
//! out — the entry's own first, then the pool's, then the table's. Conditions gate at every level.

pub mod entry;
pub mod function;
pub mod stack;

use entry::{Choice, Entry, EntryError};
use ferrumc_datapack::manager::FileToId;
use ferrumc_datapack::{Identifier, ResourceManager};
use ferrumc_predicates::condition::{Condition, ConditionError};
use ferrumc_predicates::number::NumberProvider;
use ferrumc_predicates::LootContext;
use function::{Function, FunctionError};
use serde_json::Value;
use stack::ItemStack;
use std::collections::BTreeMap;
use tracing::{error, warn};

pub use stack::ItemStack as LootStack;

/// Where a pack keeps its loot tables.
pub const DIRECTORY: &str = "loot_table";

/// Why a table could not be read.
#[derive(Debug, thiserror::Error)]
pub enum LootError {
    #[error("loot table is not an object")]
    NotAnObject,
    #[error("loot pool is missing or malformed: {0}")]
    BadPool(String),
    #[error(transparent)]
    Entry(#[from] EntryError),
    #[error(transparent)]
    Condition(#[from] ConditionError),
    #[error(transparent)]
    Function(#[from] FunctionError),
}

/// One pool of a table: what it can draw, how often, and what gates it.
#[derive(Clone, Debug)]
pub struct LootPool {
    pub entries: Vec<Entry>,
    pub conditions: Vec<Condition>,
    pub functions: Vec<Function>,
    pub rolls: NumberProvider,
    /// Extra rolls for the lucky, worked out against their luck.
    pub bonus_rolls: NumberProvider,
}

impl LootPool {
    fn parse(value: &Value) -> Result<Self, LootError> {
        let object = value
            .as_object()
            .ok_or_else(|| LootError::BadPool("not an object".to_owned()))?;
        Ok(Self {
            entries: object
                .get("entries")
                .and_then(Value::as_array)
                .ok_or_else(|| LootError::BadPool("entries".to_owned()))?
                .iter()
                .map(Entry::parse)
                .collect::<Result<_, _>>()?,
            conditions: conditions(object.get("conditions"))?,
            functions: functions(object.get("functions"))?,
            rolls: object
                .get("rolls")
                .and_then(NumberProvider::parse)
                .ok_or_else(|| LootError::BadPool("rolls".to_owned()))?,
            bonus_rolls: object
                .get("bonus_rolls")
                .and_then(NumberProvider::parse)
                .unwrap_or(NumberProvider::Constant(0.0)),
        })
    }

    fn roll(&self, context: &mut LootContext, tables: &LootTables) -> Vec<ItemStack> {
        let mut produced = Vec::new();
        if !self
            .conditions
            .iter()
            .all(|condition| condition.test(context))
        {
            return produced;
        }
        let luck = context.params.luck;
        let count =
            self.rolls.int(context) + (self.bonus_rolls.float(context) * luck).floor() as i32;

        for _ in 0..count {
            let mut choices = Vec::new();
            for entry in &self.entries {
                entry.expand(context, &mut choices);
            }
            // An entry with no weight left is out of the running rather than never drawn.
            choices.retain(|choice| choice.weight > 0);
            let Some(drawn) = draw(context, &choices) else {
                continue;
            };
            // Drawing borrows the list, and producing needs the context, so what was drawn is
            // copied out before anything is made of it.
            let drawn = *drawn;
            for stack in drawn.produce(context, tables) {
                let mut stack = stack;
                for function in &self.functions {
                    stack = function.apply(context, stack);
                }
                produced.push(stack);
            }
        }
        produced
    }
}

/// Picks one of the choices, each as likely as its weight.
fn draw<'a>(context: &mut LootContext, choices: &'a [Choice<'a>]) -> Option<&'a Choice<'a>> {
    let total: i32 = choices.iter().map(|choice| choice.weight).sum();
    if total <= 0 {
        return None;
    }
    if choices.len() == 1 {
        return choices.first();
    }
    let mut index = context.next_int(0, total - 1);
    for choice in choices {
        index -= choice.weight;
        if index < 0 {
            return Some(choice);
        }
    }
    None
}

/// A whole table.
#[derive(Clone, Debug, Default)]
pub struct LootTable {
    pub pools: Vec<LootPool>,
    pub functions: Vec<Function>,
}

impl LootTable {
    pub fn parse(value: &Value) -> Result<Self, LootError> {
        let object = value.as_object().ok_or(LootError::NotAnObject)?;
        Ok(Self {
            pools: object
                .get("pools")
                .and_then(Value::as_array)
                .map(|pools| pools.iter().map(LootPool::parse).collect())
                .unwrap_or_else(|| Ok(Vec::new()))?,
            functions: functions(object.get("functions"))?,
        })
    }
}

/// Every table the loaded packs declare.
#[derive(Debug, Default)]
pub struct LootTables {
    by_name: BTreeMap<String, LootTable>,
}

impl LootTables {
    /// Reads every loot table in a pack stack.
    #[must_use]
    pub fn load(manager: &ResourceManager) -> Self {
        let mut by_name = BTreeMap::new();
        for (id, resource) in FileToId::json(DIRECTORY).list(manager) {
            match serde_json::from_slice(&resource.data)
                .map_err(|e| e.to_string())
                .and_then(|value: Value| LootTable::parse(&value).map_err(|e| e.to_string()))
            {
                Ok(table) => {
                    by_name.insert(id.as_str().to_owned(), table);
                }
                Err(e) => error!(
                    "couldn't read loot table {id} from data pack {}: {e}",
                    resource.source
                ),
            }
        }
        Self { by_name }
    }

    /// Adds a table, which is how a test builds one without a pack behind it.
    pub fn insert(&mut self, name: Identifier, table: LootTable) {
        self.by_name.insert(name.as_str().to_owned(), table);
    }

    #[must_use]
    pub fn get(&self, name: &Identifier) -> Option<&LootTable> {
        self.by_name.get(name.as_str())
    }

    #[must_use]
    pub fn get_by_name(&self, name: &str) -> Option<&LootTable> {
        self.by_name.get(name)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    /// Rolls a table by name, collecting what it drops.
    ///
    /// Stacks of nothing are left out: a table says a drop failed by producing one, and every
    /// caller would otherwise have to weed them out itself.
    #[must_use]
    pub fn roll(&self, name: &Identifier, context: &mut LootContext) -> Vec<ItemStack> {
        let mut stacks = self.roll_raw(name, context);
        stacks.retain(|stack| !stack.is_empty());
        stacks
    }

    /// The same, empty stacks included, which is what a table nested in another produces.
    #[must_use]
    pub fn roll_raw(&self, name: &Identifier, context: &mut LootContext) -> Vec<ItemStack> {
        let Some(table) = self.get(name) else {
            warn!("tried to roll unknown loot table {name}");
            return Vec::new();
        };
        if context.visiting.iter().any(|seen| seen == name.as_str()) {
            warn!("detected infinite loop in loot tables");
            return Vec::new();
        }
        context.visiting.push(name.as_str().to_owned());

        let mut produced = Vec::new();
        for pool in &table.pools {
            for stack in pool.roll(context, self) {
                let mut stack = stack;
                for function in &table.functions {
                    stack = function.apply(context, stack);
                }
                produced.push(stack);
            }
        }

        context.visiting.pop();
        produced
    }
}

/// Reads a `conditions` list, which every level of a table may carry.
pub(crate) fn conditions(value: Option<&Value>) -> Result<Vec<Condition>, ConditionError> {
    value
        .and_then(Value::as_array)
        .map(|conditions| conditions.iter().map(Condition::parse).collect())
        .unwrap_or_else(|| Ok(Vec::new()))
}

/// The same for `functions`.
pub(crate) fn functions(value: Option<&Value>) -> Result<Vec<Function>, FunctionError> {
    value
        .and_then(Value::as_array)
        .map(|functions| functions.iter().map(Function::parse).collect())
        .unwrap_or_else(|| Ok(Vec::new()))
}

#[cfg(test)]
mod tests;
