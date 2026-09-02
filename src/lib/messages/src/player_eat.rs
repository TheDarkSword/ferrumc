use bevy_ecs::prelude::{Entity, Message};
use ferrumc_inventories::item::ItemID;

/// Something finished eating or drinking.
///
/// * Fired by: whatever finishes using an item.
/// * Listened for by: the hunger system, which feeds the eater and applies whatever the item does
///   to them afterwards.
///
/// Only what was eaten travels here. What it is worth in food, in saturation and in effects is the
/// item's own answer, and reading it in one place is what keeps the three from drifting apart.
#[derive(Message)]
pub struct PlayerEating {
    pub player: Entity,
    pub item: ItemID,
}
