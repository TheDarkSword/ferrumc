use crate::packet_handlers::player::update_crafting::update_player_crafting_grid;
use bevy_ecs::prelude::{Query, Res};
use ferrumc_inventories::defined_slots;
use ferrumc_inventories::inventory::Inventory;
use ferrumc_inventories::item::ItemID;
use ferrumc_inventories::slot::InventorySlot;
use ferrumc_net::connection::StreamWriter;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::registry_remap::item_from;
use ferrumc_net_codec::version::ProtocolVersion;
use tracing::{error, warn};

pub fn handle(
    receiver: Res<ferrumc_net::ClickContainerReceiver>,
    mut inventories: Query<&mut Inventory>,
    connections: Query<&ferrumc_net::connection::StreamWriter>,
    datapacks: Res<crate::systems::datapacks::Datapacks>,
) {
    for (event, eid) in receiver.0.try_iter() {
        // A client names an item by the number its own version gives it, which is not this
        // server's number for anything a version apart. Taking one at face value stores a
        // different item entirely.
        let speaks = connections
            .get(eid)
            .map_or(ProtocolVersion::CURRENT, StreamWriter::protocol_version);
        // TODO: actually verify that the inventory is synced, this code assumes that the ClickContainer packet is 100% truthful
        // TODO: when actually implementing this correctly, make sure that if the client sends an out of bounds slot id the entire server doesnt crash

        if let Ok(mut inventory) = inventories.get_mut(eid) {
            for slot in event.changed_slots.data {
                if let Some(new_data) = slot.data.to_option() {
                    let Some(named) = u32::try_from(new_data.item_id.0)
                        .ok()
                        .and_then(|id| item_from(id, speaks))
                        .and_then(|id| i32::try_from(id).ok())
                    else {
                        warn!(
                            "a {speaks:?} client named the item {}, which is nothing this server has",
                            new_data.item_id.0
                        );
                        continue;
                    };
                    let named = ItemID(VarInt::new(named));
                    // A client sends its components as hashes rather than as values, so there is
                    // nothing here to rebuild them from. Whatever the server already had in the
                    // slot is kept where the kind still matches, so a named sword moved across an
                    // inventory does not come out plain.
                    let held = inventory
                        .get_item(slot.number as usize)
                        .ok()
                        .flatten()
                        .filter(|held| held.item_id == Some(named))
                        .map(|held| held.components.clone())
                        .unwrap_or_default();
                    inventory
                        .set_item(
                            slot.number as _,
                            InventorySlot {
                                count: new_data.item_count,
                                item_id: Some(named),
                                components: held,
                            },
                        )
                        .expect("failed to write to inventory");
                } else {
                    inventory
                        .clear_slot_with_update(slot.number as _, eid)
                        .expect("failed to clear item in inventory");
                }

                if (defined_slots::player::CRAFT_SLOT_1..=defined_slots::player::CRAFT_SLOT_4)
                    .contains(&(slot.number as u8))
                {
                    update_player_crafting_grid(&mut inventory, eid, &datapacks.recipes);
                }
            }
        } else {
            error!("Failed to get inventory for entity {eid}");
        }
    }
}
