//! The condition language the game is written in.
//!
//! Loot tables, advancements and functions all gate on the same thing: a predicate, evaluated
//! against a bag of parameters that says what is going on — the block being broken, the tool that
//! broke it, who swung it, where. Vanilla splits the two halves across `advancements/predicates`
//! and `loot/predicates`; they are one language and live together here.

pub mod block;
pub mod bounds;
pub mod condition;
pub mod context;
pub mod holders;
pub mod item;
pub mod location;
pub mod number;
pub mod state;

pub use block::BlockPredicate;
pub use bounds::Bounds;
pub use condition::{Condition, Predicates};
pub use context::{LootContext, LootParams, LootWorld};
pub use holders::HolderSet;
pub use item::ItemPredicate;
pub use location::LocationPredicate;
pub use state::StateProperties;

#[cfg(test)]
mod tests;
