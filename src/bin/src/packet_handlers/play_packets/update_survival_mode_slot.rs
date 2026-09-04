//! What a client says it did to a container.
//!
//! Two things a client sends here cannot be taken at face value. The first is the item's number,
//! which is its own version's and not this server's. The second is the components, which a client
//! sends as **hashes** rather than values — there is nothing in the packet to rebuild a name or an
//! enchantment from.
//!
//! So the components are not read off the packet at all: they are carried across from wherever they
//! were before the click. A click moves stacks around inside one inventory, so what left one of the
//! named slots is what arrives in another, and matching them by kind puts each back on its stack.
//! Without that, moving an enchanted sword one slot to the left would leave a plain one behind.

use crate::packet_handlers::player::update_crafting::{
    take_one_round, update_player_crafting_grid,
};
use bevy_ecs::prelude::{Query, Res};
use ferrumc_inventories::components::Components;
use ferrumc_inventories::defined_slots;
use ferrumc_inventories::inventory::Inventory;
use ferrumc_inventories::item::ItemID;
use ferrumc_inventories::slot::InventorySlot;
use ferrumc_net::connection::StreamWriter;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::registry_remap::item_from;
use ferrumc_net_codec::version::ProtocolVersion;
use tracing::{error, warn};

/// What was on the stacks a click is moving about, waiting to be put back on them.
///
/// Kept as a list rather than a map because two stacks of the same kind may carry different things
/// — two named swords are two names — and each is handed out once.
#[derive(Default)]
pub struct InFlight(Vec<(ItemID, Components)>);

impl InFlight {
    /// Everything worth carrying that is currently in the slots a click names.
    #[must_use]
    pub fn leaving(inventory: &Inventory, slots: impl Iterator<Item = usize>) -> Self {
        Self(
            slots
                .filter_map(|slot| inventory.get_item(slot).ok().flatten())
                .filter(|held| !held.components.is_empty())
                .filter_map(|held| Some((held.item_id?, held.components.clone())))
                .collect(),
        )
    }

    /// Takes back what belonged to a stack of this kind, if anything did.
    ///
    /// Each is handed out once, so two swords of the same kind do not both end up with the first
    /// one's name.
    pub fn claim(&mut self, kind: ItemID) -> Option<Components> {
        let at = self.0.iter().position(|(held, _)| *held == kind)?;
        Some(self.0.remove(at).1)
    }
}

pub fn handle(
    receiver: Res<ferrumc_net::ClickContainerReceiver>,
    mut inventories: Query<&mut Inventory>,
    connections: Query<&StreamWriter>,
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

        let Ok(mut inventory) = inventories.get_mut(eid) else {
            error!("Player {eid:?} sent a container click but has no inventory");
            continue;
        };

        // Read before anything is written: once a slot has been overwritten, what was on it is
        // gone, and a click writes the destination before the source.
        let mut in_flight = InFlight::leaving(
            &inventory,
            event
                .changed_slots
                .data
                .iter()
                .map(|slot| slot.number as usize),
        );

        // Taking the result is the one click the server carries out itself rather than believing.
        // A client that says the grid emptied and the result appeared is describing a trade, and
        // believing both halves is how one plank becomes a crafting table and a plank again.
        let took_the_result = event
            .changed_slots
            .data
            .iter()
            .any(|slot| slot.number == i16::from(defined_slots::player::CRAFT_SLOT_OUTPUT));
        let spilled = if took_the_result && inventory.crafting_output_is_set() {
            take_one_round(&mut inventory, eid)
        } else {
            Vec::new()
        };

        for slot in event.changed_slots.data {
            // The grid was just spent by the server; what the client says about it is a report of
            // the same trade and would spend it twice.
            if took_the_result && GRID_AND_OUTPUT.contains(&(slot.number as u8)) {
                continue;
            }
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

                inventory
                    .set_item(
                        slot.number as _,
                        InventorySlot {
                            count: new_data.item_count,
                            item_id: Some(named),
                            components: in_flight.claim(named).unwrap_or_default(),
                        },
                    )
                    .expect("failed to write to inventory");
            } else if let Err(e) = inventory.clear_slot_with_update(slot.number as _, eid) {
                error!(
                    "Failed to clear item in slot {} for player {eid:?}: {e:?}",
                    slot.number
                );
            }

            if (defined_slots::player::CRAFT_SLOT_1..=defined_slots::player::CRAFT_SLOT_4)
                .contains(&(slot.number as u8))
            {
                update_player_crafting_grid(&mut inventory, eid, &datapacks.recipes);
            }
        }

        if took_the_result {
            // What the grid now makes, if anything: holding shift on a stack of planks should keep
            // producing.
            update_player_crafting_grid(&mut inventory, eid, &datapacks.recipes);
            // And anything a spent ingredient left behind with nowhere to go.
            for left in spilled {
                let over = inventory.add_item(left, None);
                if !over.is_empty() {
                    warn!("a crafting remainder had nowhere to go and was lost");
                }
            }
        }
    }
}

/// The grid and the slot its result sits in.
const GRID_AND_OUTPUT: [u8; 5] = [
    defined_slots::player::CRAFT_SLOT_OUTPUT,
    defined_slots::player::CRAFT_SLOT_1,
    defined_slots::player::CRAFT_SLOT_2,
    defined_slots::player::CRAFT_SLOT_3,
    defined_slots::player::CRAFT_SLOT_4,
];

#[cfg(test)]
mod tests {
    use super::*;
    use ferrumc_data::generated::components::ComponentType;
    use ferrumc_inventories::components::Value;
    use ferrumc_text::ComponentBuilder;

    const SWORD: ItemID = ItemID(VarInt::new(895));
    const STONE: ItemID = ItemID(VarInt::new(1));

    fn an_inventory_holding_a_named_sword(at: usize) -> Inventory {
        let mut inventory = Inventory::new(Inventory::DEFAULT_PLAYER_SIZE);
        let mut sword = InventorySlot::of(SWORD, 1);
        sword
            .components
            .set_name(&ComponentBuilder::text("Sting").build());
        sword
            .components
            .set(ComponentType::Damage, Value::Number(12));
        inventory.set_item(at, sword).expect("the slot exists");
        inventory
    }

    /// The thing this is here for: a click moves a stack, and its name has to move with it.
    #[test]
    fn a_named_sword_moved_across_the_inventory_keeps_its_name() {
        let inventory = an_inventory_holding_a_named_sword(36);
        let mut in_flight = InFlight::leaving(&inventory, [36, 5].into_iter());

        let landed = in_flight.claim(SWORD).expect("the sword's own components");
        assert_eq!(
            landed.get(ComponentType::Damage),
            Some(&Value::Number(12)),
            "and how damaged it was"
        );
        assert!(landed.get(ComponentType::CustomName).is_some());
    }

    /// Handed out once, so a second sword does not inherit the first one's name.
    #[test]
    fn two_swords_do_not_both_get_the_first_ones_name() {
        let inventory = an_inventory_holding_a_named_sword(36);
        let mut in_flight = InFlight::leaving(&inventory, [36].into_iter());

        assert!(in_flight.claim(SWORD).is_some());
        assert!(in_flight.claim(SWORD).is_none());
    }

    #[test]
    fn a_stack_of_another_kind_claims_nothing() {
        let inventory = an_inventory_holding_a_named_sword(36);
        let mut in_flight = InFlight::leaving(&inventory, [36].into_iter());
        assert!(in_flight.claim(STONE).is_none());
    }

    #[test]
    fn a_plain_stack_carries_nothing_and_is_not_in_flight() {
        let mut inventory = Inventory::new(Inventory::DEFAULT_PLAYER_SIZE);
        inventory
            .set_item(36, InventorySlot::of(STONE, 64))
            .expect("the slot exists");
        let mut in_flight = InFlight::leaving(&inventory, [36].into_iter());
        assert!(in_flight.claim(STONE).is_none());
    }
}
