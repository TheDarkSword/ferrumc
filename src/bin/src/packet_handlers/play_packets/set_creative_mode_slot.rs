use bevy_ecs::prelude::{Query, Res};
use ferrumc_inventories::inventory::Inventory;
use ferrumc_inventories::item::ItemID;
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::SetCreativeModeSlotReceiver;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::registry_remap::item_from;
use ferrumc_net_codec::version::ProtocolVersion;
use ferrumc_state::GlobalStateResource;
use tracing::{debug, error, warn};

pub fn handle(
    receiver: Res<SetCreativeModeSlotReceiver>,
    state: Res<GlobalStateResource>,
    mut query: Query<&mut Inventory>,
    connections: Query<&StreamWriter>,
) {
    for (mut event, entity) in receiver.0.try_iter() {
        debug!(
            "Slot {} placed at {} by player {}",
            event.slot, event.slot_index, entity
        );
        // A client names an item by the number its own version gives it. The two only agree for a
        // client speaking this server's own version, and taking one at face value in creative is
        // how a player asks for one thing and is handed another.
        let speaks = connections
            .get(entity)
            .map_or(ProtocolVersion::CURRENT, StreamWriter::protocol_version);
        if let Some(named) = event.slot.item_id {
            let Some(ours) = u32::try_from(named.0 .0)
                .ok()
                .and_then(|id| item_from(id, speaks))
                .and_then(|id| i32::try_from(id).ok())
            else {
                warn!(
                    "a {speaks:?} client asked for the item {}, which is nothing this server has",
                    named.0 .0
                );
                continue;
            };
            event.slot.item_id = Some(ItemID(VarInt::new(ours)));
        }

        if state.0.players.is_connected(entity) {
            if let Ok(mut inventory) = query.get_mut(entity) {
                if event.slot.count.0 == 0 {
                    if let Err(e) =
                        inventory.clear_slot_with_update(event.slot_index as usize, entity)
                    {
                        error!(
                            "Failed to clear slot {} for player {}: {:?}",
                            event.slot_index, entity, e
                        );
                    }
                } else if let Err(e) =
                    inventory.set_item_with_update(event.slot_index as usize, event.slot, entity)
                {
                    error!(
                        "Failed to set item in slot {} for player {}: {:?}",
                        event.slot_index, entity, e
                    );
                }
            }
        }
    }
}
