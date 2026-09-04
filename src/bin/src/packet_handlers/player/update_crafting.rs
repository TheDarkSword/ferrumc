use bevy_ecs::prelude::Entity;
use ferrumc_data::generated::items::crafting_remainder;
use ferrumc_inventories::defined_slots;
use ferrumc_inventories::inventory::Inventory;
use ferrumc_inventories::item::ItemID;
use ferrumc_inventories::slot::InventorySlot;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_recipes::crafting::CraftingInput;
use ferrumc_recipes::{Recipe, RecipeBook};
use tracing::error;

/// Works out what the player's two-by-two grid currently makes, and puts it in the output slot.
pub fn update_player_crafting_grid(inventory: &mut Inventory, eid: Entity, recipes: &RecipeBook) {
    let grid = GRID.map(|slot| item_in(inventory, slot));

    let tags = ferrumc_registry::tags::current().item();
    let made = CraftingInput::new(2, 2, &grid);
    let made = recipes.match_grid(&tags, &made).and_then(Recipe::result);

    let Some(made) = made else {
        inventory
            .clear_slot_with_update(defined_slots::player::CRAFT_SLOT_OUTPUT as _, eid)
            .unwrap_or_else(|err| error!("Failed to clear player crafting output slot: {}", err));
        return;
    };

    let slot = InventorySlot {
        item_id: Some(ItemID(VarInt(made.item))),
        count: VarInt(made.count),
        ..Default::default()
    };
    inventory
        .set_item_with_update(defined_slots::player::CRAFT_SLOT_OUTPUT as _, slot, eid)
        .unwrap_or_else(|err| error!("Failed to set player crafting output slot: {}", err));
}

/// Takes one round of ingredients out of the grid, and puts back what each leaves behind.
///
/// This is what makes crafting real rather than reported. A client says the grid emptied and the
/// result appeared; believing it is how one plank becomes a crafting table and a crafting table
/// again. Instead the server spends the grid itself the moment the result is taken.
///
/// A bucket of milk leaves the bucket, in the slot it came from where that slot is now empty and on
/// the floor otherwise — vanilla puts it in the player's hands, which needs somewhere to put it.
pub fn take_one_round(inventory: &mut Inventory, eid: Entity) -> Vec<InventorySlot> {
    let mut spilled = Vec::new();
    for slot in GRID {
        let at = usize::from(slot);
        let Ok(Some(mut held)) = inventory.get_item(at).map(|held| held.cloned()) else {
            continue;
        };
        if held.is_empty() {
            continue;
        }

        // What this ingredient leaves behind, worked out before it is spent.
        let left = held
            .item_id
            .and_then(|kind| u16::try_from(kind.0 .0).ok())
            .and_then(crafting_remainder)
            .map(|left| InventorySlot::of(ItemID(VarInt::new(i32::from(left))), 1));

        held.count.0 -= 1;
        let now = if held.count.0 > 0 {
            held
        } else {
            InventorySlot::empty()
        };

        match (now.is_empty(), left) {
            // The slot emptied and something is left behind: it takes the slot.
            (true, Some(left)) => {
                let _ = inventory.set_item_with_update(at, left, eid);
            }
            (true, None) => {
                let _ = inventory.clear_slot_with_update(at, eid);
            }
            // The slot still holds more of the same, so what is left behind has nowhere to go here.
            (false, left) => {
                let _ = inventory.set_item_with_update(at, now, eid);
                spilled.extend(left);
            }
        }
    }
    spilled
}

/// The four slots the grid is made of.
const GRID: [u8; 4] = [
    defined_slots::player::CRAFT_SLOT_1,
    defined_slots::player::CRAFT_SLOT_2,
    defined_slots::player::CRAFT_SLOT_3,
    defined_slots::player::CRAFT_SLOT_4,
];

/// The registry id of whatever is in a slot.
fn item_in(inventory: &Inventory, slot: u8) -> Option<i32> {
    inventory
        .get_item(slot as usize)
        .ok()
        .flatten()
        .and_then(|slot| slot.item_id)
        .map(|item| item.0 .0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrumc_inventories::defined_slots::player;

    fn id(name: &str) -> ItemID {
        ItemID(VarInt::new(i32::from(
            ferrumc_data::generated::items::Item::from_registry_key(name)
                .expect("it is an item")
                .id,
        )))
    }

    fn a_grid_holding(items: &[(u8, ItemID, i32)]) -> Inventory {
        let mut inventory = Inventory::new(Inventory::DEFAULT_PLAYER_SIZE);
        for (slot, kind, count) in items {
            inventory
                .set_item(usize::from(*slot), InventorySlot::of(*kind, *count))
                .expect("the slot exists");
        }
        inventory
    }

    /// Somewhere to send the updates to. Nothing here reads them.
    fn nobody() -> Entity {
        bevy_ecs::world::World::new().spawn_empty().id()
    }

    fn count_in(inventory: &Inventory, at: u8) -> i32 {
        inventory
            .get_item(usize::from(at))
            .ok()
            .flatten()
            .map_or(0, |held| held.count.0)
    }

    fn kind_in(inventory: &Inventory, at: u8) -> Option<ItemID> {
        inventory
            .get_item(usize::from(at))
            .ok()
            .flatten()
            .and_then(|held| held.item_id)
    }

    /// One round takes one of each, not the whole stack.
    #[test]
    fn taking_a_result_spends_one_of_each_ingredient() {
        let planks = id("minecraft:oak_planks");
        let mut inventory = a_grid_holding(&[
            (player::CRAFT_SLOT_1, planks, 10),
            (player::CRAFT_SLOT_2, planks, 10),
            (player::CRAFT_SLOT_3, planks, 10),
            (player::CRAFT_SLOT_4, planks, 10),
        ]);

        take_one_round(&mut inventory, nobody());
        for slot in GRID {
            assert_eq!(count_in(&inventory, slot), 9, "one of each went");
        }
    }

    #[test]
    fn the_last_of_an_ingredient_empties_its_slot() {
        let planks = id("minecraft:oak_planks");
        let mut inventory = a_grid_holding(&[(player::CRAFT_SLOT_1, planks, 1)]);

        take_one_round(&mut inventory, nobody());
        assert_eq!(count_in(&inventory, player::CRAFT_SLOT_1), 0);
    }

    /// A bucket of milk leaves the bucket, in the slot it came from.
    #[test]
    fn a_bucket_stays_behind() {
        let milk = id("minecraft:milk_bucket");
        let mut inventory = a_grid_holding(&[(player::CRAFT_SLOT_1, milk, 1)]);

        let spilled = take_one_round(&mut inventory, nobody());
        assert!(spilled.is_empty(), "it had somewhere to go");
        assert_eq!(
            kind_in(&inventory, player::CRAFT_SLOT_1),
            Some(id("minecraft:bucket"))
        );
    }

    /// And where the slot still holds more, the bucket has nowhere to go and is handed back.
    #[test]
    fn a_bucket_with_nowhere_to_go_is_handed_back() {
        let milk = id("minecraft:milk_bucket");
        let mut inventory = a_grid_holding(&[(player::CRAFT_SLOT_1, milk, 2)]);

        let spilled = take_one_round(&mut inventory, nobody());
        assert_eq!(count_in(&inventory, player::CRAFT_SLOT_1), 1, "one is left");
        assert_eq!(spilled.len(), 1);
        assert_eq!(spilled[0].item_id, Some(id("minecraft:bucket")));
    }

    #[test]
    fn an_empty_grid_spends_nothing() {
        let mut inventory = a_grid_holding(&[]);
        assert!(take_one_round(&mut inventory, nobody()).is_empty());
    }

    /// A slot outside the grid is not touched, however full it is.
    #[test]
    fn nothing_outside_the_grid_is_spent() {
        let planks = id("minecraft:oak_planks");
        let mut inventory = a_grid_holding(&[
            (player::CRAFT_SLOT_1, planks, 5),
            (player::HOTBAR_SLOT_1, planks, 64),
        ]);

        take_one_round(&mut inventory, nobody());
        assert_eq!(count_in(&inventory, player::HOTBAR_SLOT_1), 64);
    }
}
