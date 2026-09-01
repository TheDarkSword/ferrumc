//! Draining packets nothing acts on yet.
//!
//! Every serverbound packet gets a channel whether or not a system reads it, and a channel nobody
//! reads grows for as long as the server runs. Some of these arrive every tick from every player,
//! so this is a leak rather than a tidiness matter.
//!
//! A packet leaves this list when it gets a handler of its own.

use bevy_ecs::prelude::Res;
use ferrumc_net::{
    ChangeDifficultyReceiver, ChatCommandSignedReceiver, ClientTickEndPacketReceiver,
    InteractEntityReceiver, MovePlayerStatusOnlyReceiver, MoveVehicleReceiver,
    PickItemFromEntityReceiver, PlaceRecipeReceiver, RecipeBookSeenRecipeReceiver,
    RenameItemReceiver, UseItemReceiver,
};

/// Empties every channel with no reader. `try_iter` takes what is queued and returns.
#[expect(clippy::too_many_arguments)]
pub fn handle(
    change_difficulty: Res<ChangeDifficultyReceiver>,
    chat_command_signed: Res<ChatCommandSignedReceiver>,
    client_tick_end: Res<ClientTickEndPacketReceiver>,
    interact: Res<InteractEntityReceiver>,
    move_player_status_only: Res<MovePlayerStatusOnlyReceiver>,
    move_vehicle: Res<MoveVehicleReceiver>,
    pick_item_from_entity: Res<PickItemFromEntityReceiver>,
    place_recipe: Res<PlaceRecipeReceiver>,
    recipe_book_seen_recipe: Res<RecipeBookSeenRecipeReceiver>,
    rename_item: Res<RenameItemReceiver>,
    use_item: Res<UseItemReceiver>,
) {
    change_difficulty.0.try_iter().for_each(drop);
    chat_command_signed.0.try_iter().for_each(drop);
    client_tick_end.0.try_iter().for_each(drop);
    interact.0.try_iter().for_each(drop);
    move_player_status_only.0.try_iter().for_each(drop);
    move_vehicle.0.try_iter().for_each(drop);
    pick_item_from_entity.0.try_iter().for_each(drop);
    place_recipe.0.try_iter().for_each(drop);
    recipe_book_seen_recipe.0.try_iter().for_each(drop);
    rename_item.0.try_iter().for_each(drop);
    use_item.0.try_iter().for_each(drop);
}
