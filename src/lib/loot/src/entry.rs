//! What a pool can pick, and how a pick is worked out.
//!
//! An entry either produces something itself or gathers others. Expanding one asks it which of its
//! parts are in the running this time: a singleton puts itself forward if its conditions hold, an
//! `alternatives` puts forward the first of its children that does, a `group` puts forward all of
//! them. What comes back is a weighted list, and the pool draws from it.

use crate::function::{Function, FunctionError};
use crate::stack::ItemStack;
use ferrumc_datapack::Identifier;
use ferrumc_predicates::condition::{Condition, ConditionError};
use ferrumc_predicates::LootContext;
use serde_json::Value;
use tracing::warn;

/// Why an entry could not be read.
#[derive(Debug, thiserror::Error)]
pub enum EntryError {
    #[error("entry is not an object")]
    NotAnObject,
    #[error("entry has no type")]
    NoType,
    #[error("unknown entry type '{0}'")]
    UnknownType(String),
    #[error("entry '{kind}' is missing or malformed: {field}")]
    BadField { kind: String, field: String },
    #[error(transparent)]
    Condition(#[from] ConditionError),
    #[error(transparent)]
    Function(#[from] FunctionError),
}

/// What every entry carries, gathering or not.
#[derive(Clone, Debug, Default)]
pub struct Common {
    pub conditions: Vec<Condition>,
    pub functions: Vec<Function>,
    /// How likely this is to be drawn against its siblings.
    pub weight: i32,
    /// How much luck shifts that weight.
    pub quality: i32,
}

impl Common {
    /// What this entry is worth to someone this lucky. Vanilla floors and refuses to go below
    /// nought, so a negative quality can put an entry out of the running rather than into debt.
    #[must_use]
    pub fn weight(&self, luck: f32) -> i32 {
        #[expect(clippy::cast_precision_loss)]
        let weight = self.weight as f32 + self.quality as f32 * luck;
        (weight.floor() as i32).max(0)
    }
}

#[derive(Clone, Debug)]
pub enum Entry {
    /// One item.
    Item { item: i32, common: Common },
    /// Nothing, which is how a table says a drop may fail.
    Empty { common: Common },
    /// Another table, rolled in place.
    Nested { table: Identifier, common: Common },
    /// Every item in a tag, either as one entry each or all at once.
    Tag {
        tag: Identifier,
        expand: bool,
        common: Common,
    },
    /// The first child that can run.
    Alternatives {
        children: Vec<Entry>,
        conditions: Vec<Condition>,
    },
    /// Every child, until one cannot run.
    Sequence {
        children: Vec<Entry>,
        conditions: Vec<Condition>,
    },
    /// Every child, whatever they say.
    Group {
        children: Vec<Entry>,
        conditions: Vec<Condition>,
    },
    /// An entry whose source does not exist yet. Never in the running, and says so once.
    Unsupported { kind: &'static str, common: Common },
}

/// One thing that could be drawn, and what it is worth.
#[derive(Clone, Copy)]
pub struct Choice<'a> {
    pub weight: i32,
    entry: &'a Entry,
    /// Which member of a tag, where the entry is an expanded one.
    member: Option<i32>,
}

impl Entry {
    pub fn parse(value: &Value) -> Result<Self, EntryError> {
        let object = value.as_object().ok_or(EntryError::NotAnObject)?;
        let kind = object
            .get("type")
            .and_then(Value::as_str)
            .ok_or(EntryError::NoType)?;
        let bare = kind.strip_prefix("minecraft:").unwrap_or(kind);
        let bad = |field: &str| EntryError::BadField {
            kind: kind.to_owned(),
            field: field.to_owned(),
        };
        let conditions = crate::conditions(object.get("conditions"))?;
        let common = || -> Result<Common, EntryError> {
            Ok(Common {
                conditions: crate::conditions(object.get("conditions"))?,
                functions: crate::functions(object.get("functions"))?,
                weight: object
                    .get("weight")
                    .and_then(Value::as_i64)
                    .and_then(|w| i32::try_from(w).ok())
                    .unwrap_or(1),
                quality: object
                    .get("quality")
                    .and_then(Value::as_i64)
                    .and_then(|q| i32::try_from(q).ok())
                    .unwrap_or_default(),
            })
        };
        let children = || -> Result<Vec<Entry>, EntryError> {
            object
                .get("children")
                .and_then(Value::as_array)
                .map(|children| children.iter().map(Self::parse).collect())
                .unwrap_or_else(|| Ok(Vec::new()))
        };

        Ok(match bare {
            "item" => Self::Item {
                item: crate::function::named_item(object.get("name")).ok_or_else(|| bad("name"))?,
                common: common()?,
            },
            "empty" => Self::Empty { common: common()? },
            "loot_table" => {
                // The value is either the name of a table or one written out in place; only the
                // named form is followed here.
                let value = object.get("value").ok_or_else(|| bad("value"))?;
                match value.as_str() {
                    Some(name) => Self::Nested {
                        table: Identifier::parse(name).map_err(|_| bad("value"))?,
                        common: common()?,
                    },
                    None => Self::Unsupported {
                        kind: "loot_table written in place",
                        common: common()?,
                    },
                }
            }
            "tag" => Self::Tag {
                tag: object
                    .get("name")
                    .and_then(Value::as_str)
                    .and_then(|name| Identifier::parse(name).ok())
                    .ok_or_else(|| bad("name"))?,
                expand: object
                    .get("expand")
                    .and_then(Value::as_bool)
                    .unwrap_or_default(),
                common: common()?,
            },
            "alternatives" => Self::Alternatives {
                children: children()?,
                conditions,
            },
            "sequence" => Self::Sequence {
                children: children()?,
                conditions,
            },
            "group" => Self::Group {
                children: children()?,
                conditions,
            },
            // Container slots and block-entity drops both need something to read the contents of.
            "slots" => Self::Unsupported {
                kind: "slots",
                common: common()?,
            },
            "dynamic" => Self::Unsupported {
                kind: "dynamic",
                common: common()?,
            },
            _ => return Err(EntryError::UnknownType(kind.to_owned())),
        })
    }

    /// Puts whatever of this entry is in the running into `out`.
    ///
    /// The answer says whether anything was, which is what `alternatives` stops on and `sequence`
    /// gives up on.
    pub fn expand<'a>(&'a self, context: &mut LootContext, out: &mut Vec<Choice<'a>>) -> bool {
        match self {
            Self::Alternatives {
                children,
                conditions,
            } => {
                if !holds(conditions, context) {
                    return false;
                }
                children.iter().any(|child| child.expand(context, out))
            }
            Self::Sequence {
                children,
                conditions,
            } => {
                if !holds(conditions, context) {
                    return false;
                }
                // `all` stops at the first child that cannot run, which is what makes this a
                // sequence rather than a group.
                children.iter().all(|child| child.expand(context, out))
            }
            Self::Group {
                children,
                conditions,
            } => {
                if !holds(conditions, context) {
                    return false;
                }
                for child in children {
                    child.expand(context, out);
                }
                true
            }
            Self::Tag {
                tag,
                expand: true,
                common,
            } => {
                if !holds(&common.conditions, context) {
                    return false;
                }
                // Each member stands on its own, so a tag of ten items is ten chances rather than
                // one. Every member carries the entry's weight.
                let items = context.tags.item();
                let Some(tag) = items.get(tag) else {
                    return true;
                };
                for member in items.elements(tag) {
                    if let Ok(member) = i32::try_from(*member) {
                        out.push(Choice {
                            weight: common.weight(context.params.luck),
                            entry: self,
                            member: Some(member),
                        });
                    }
                }
                true
            }
            other => {
                let common = other.common();
                if !holds(&common.conditions, context) {
                    return false;
                }
                out.push(Choice {
                    weight: common.weight(context.params.luck),
                    entry: self,
                    member: None,
                });
                true
            }
        }
    }

    fn common(&self) -> &Common {
        match self {
            Self::Item { common, .. }
            | Self::Empty { common }
            | Self::Nested { common, .. }
            | Self::Tag { common, .. }
            | Self::Unsupported { common, .. } => common,
            // A gathering entry never stands on its own, so its weight is never asked for.
            _ => &EMPTY_COMMON,
        }
    }
}

static EMPTY_COMMON: Common = Common {
    conditions: Vec::new(),
    functions: Vec::new(),
    weight: 1,
    quality: 0,
};

impl Choice<'_> {
    /// Produces what was drawn, running the entry's own functions over it.
    ///
    /// The stacks come back as a list rather than being handed out one at a time: a function needs
    /// the context to run, and so does whatever the caller does with the stack.
    #[must_use]
    pub fn produce(&self, context: &mut LootContext, tables: &crate::LootTables) -> Vec<ItemStack> {
        let (common, stacks): (&Common, Vec<ItemStack>) = match self.entry {
            Entry::Item { item, common } => (common, vec![ItemStack::new(*item)]),
            Entry::Empty { common } => (common, Vec::new()),
            Entry::Tag { tag, common, .. } => {
                let stacks = match self.member {
                    // One member of an expanded tag.
                    Some(item) => vec![ItemStack::new(item)],
                    // The whole tag at once.
                    None => {
                        let items = context.tags.item();
                        items.get(tag).map_or_else(Vec::new, |tag| {
                            items
                                .elements(tag)
                                .iter()
                                .filter_map(|item| i32::try_from(*item).ok())
                                .map(ItemStack::new)
                                .collect()
                        })
                    }
                };
                (common, stacks)
            }
            Entry::Nested { table, common } => (common, tables.roll_raw(table, context)),
            Entry::Unsupported { kind, common } => {
                warn!("loot entry {kind} is not supported yet, dropping nothing");
                (common, Vec::new())
            }
            // A gathering entry is never drawn: expanding it puts its children forward instead.
            _ => return Vec::new(),
        };

        stacks
            .into_iter()
            .map(|stack| {
                let mut stack = stack;
                for function in &common.functions {
                    stack = function.apply(context, stack);
                }
                stack
            })
            .collect()
    }
}

fn holds(conditions: &[Condition], context: &mut LootContext) -> bool {
    conditions.iter().all(|condition| condition.test(context))
}
