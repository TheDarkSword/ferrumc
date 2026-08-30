use bevy_ecs::prelude::Entity;
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
    let grid = [
        defined_slots::player::CRAFT_SLOT_1,
        defined_slots::player::CRAFT_SLOT_2,
        defined_slots::player::CRAFT_SLOT_3,
        defined_slots::player::CRAFT_SLOT_4,
    ]
    .map(|slot| item_in(inventory, slot));

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

/// The registry id of whatever is in a slot.
fn item_in(inventory: &Inventory, slot: u8) -> Option<i32> {
    inventory
        .get_item(slot as usize)
        .ok()
        .flatten()
        .and_then(|slot| slot.item_id)
        .map(|item| item.0 .0)
}
