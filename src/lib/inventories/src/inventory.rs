use crate::defined_slots::player;
use crate::errors::InventoryError;
use crate::item::ItemID;
use crate::slot::InventorySlot;
use crate::{INVENTORY_UPDATES_QUEUE, InventoryUpdate};
use bevy_ecs::prelude::{Component, Entity};
use bitcode_derive::{Decode, Encode};
use ferrumc_data::generated::items::{DataComponent, Item, MaxStackSizeImpl};

/// What almost everything stacks to, where the item says nothing else.
const DEFAULT_MAX_STACK: u8 = 64;

#[derive(Component, Clone, Debug, Decode, Encode)]
pub struct Inventory {
    pub slots: Box<[Option<InventorySlot>]>,
}

impl Default for Inventory {
    /// Make default inventory, sized for a PLAYER.
    /// 46 = (5 * 9) + 1 =
    /// NOT divisible by 9.
    fn default() -> Self {
        Self::new(Self::DEFAULT_PLAYER_SIZE)
    }
}

impl Inventory {
    pub const DEFAULT_PLAYER_SIZE: usize = 46;

    /// Where the four pieces of armour sit, head first.
    ///
    /// The layout is the wire's own and every version shares it: a crafting result, a two by two
    /// grid, the armour, the main store, the hotbar, and the off hand last.
    pub const ARMOUR_SLOTS: [usize; 4] = [5, 6, 7, 8];

    /// Where what is held in the other hand sits.
    pub const OFFHAND_SLOT: usize = 45;

    /// What each armour slot is called, in the same order, which is how an item says where it goes.
    pub const ARMOUR_SLOT_NAMES: [&'static str; 4] = ["head", "chest", "legs", "feet"];

    pub fn new(size: usize) -> Self {
        Self {
            slots: vec![None; size].into_boxed_slice(),
        }
    }

    pub fn clear(&mut self) {
        for slot in &mut self.slots {
            *slot = None;
        }
    }

    pub fn contains_item(&self, item_id: i32) -> bool {
        self.slots.iter().any(|slot| {
            if let Some(slot) = slot {
                if let Some(item) = &slot.item_id {
                    item.0.0 == item_id
                } else {
                    false
                }
            } else {
                false
            }
        })
    }

    /// The most of one kind of item that fits in a slot.
    ///
    /// Sixty-four for most things, sixteen for pearls and snowballs, one for a sword. It is a
    /// component on the item, and one a particular stack may override — a stack cannot override it
    /// yet, since nothing sets one.
    #[must_use]
    pub fn stacks_to(item: ItemID) -> u8 {
        u16::try_from(item.0.0)
            .ok()
            .and_then(Item::from_id)
            .and_then(|item| {
                item.components.iter().find_map(|(id, data)| {
                    (*id == DataComponent::MaxStackSize)
                        .then(|| data.as_any().downcast_ref::<MaxStackSizeImpl>())
                        .flatten()
                })
            })
            .map_or(DEFAULT_MAX_STACK, |held| held.size)
    }

    /// Puts as much of a stack away as will fit, and hands back whatever would not.
    ///
    /// Partial stacks of the same thing are filled before an empty slot is used — otherwise picking
    /// up a second handful of dirt makes a second stack of it. What counts as "the same thing" is
    /// the kind *and* everything the stack says about itself: a named sword does not merge into a
    /// plain one.
    ///
    /// The order is vanilla's: what is in hand first, then the off hand, then the hotbar and the
    /// main store.
    pub fn add_item(&mut self, mut item: InventorySlot, in_hand: Option<u8>) -> InventorySlot {
        if item.is_empty() {
            return InventorySlot::empty();
        }
        let Some(kind) = item.item_id else {
            return item;
        };
        let ceiling = i32::from(Self::stacks_to(kind));

        let order = self.merge_order(in_hand);

        // Filling what is already there, in the order vanilla looks.
        for at in order.iter().copied() {
            if item.count.0 <= 0 {
                return InventorySlot::empty();
            }
            let Some(Some(held)) = self.slots.get_mut(at) else {
                continue;
            };
            if !held.same_thing_as(&item) {
                continue;
            }
            let room = ceiling - held.count.0;
            if room <= 0 {
                continue;
            }
            let moved = room.min(item.count.0);
            held.count.0 += moved;
            item.count.0 -= moved;
        }

        // And then whatever is left goes somewhere empty, a stack at a time. Not the off hand:
        // vanilla will top up what is already held there and will never put something new into it.
        for at in self.placement_order() {
            if item.count.0 <= 0 {
                return InventorySlot::empty();
            }
            let Some(slot @ None) = self.slots.get_mut(at) else {
                continue;
            };
            let moved = ceiling.min(item.count.0);
            let mut put = item.clone();
            put.count.0 = moved;
            *slot = Some(put);
            item.count.0 -= moved;
        }

        if item.count.0 <= 0 {
            InventorySlot::empty()
        } else {
            item
        }
    }

    /// The slots a stack is put away into, in the order vanilla looks at them.
    ///
    /// For a player: what is in hand first, so picking something up tops up what is being held;
    /// then the off hand; then the hotbar and the main store. The armour and the crafting grid are
    /// not places things are put away, which is why they are not in the list.
    ///
    /// Where a stack that fitted nowhere goes.
    ///
    /// The same order less the off hand: vanilla tops up what is already held there and never puts
    /// something new into it, which is why a picked-up block does not land in the shield hand.
    fn placement_order(&self) -> Vec<usize> {
        if self.slots.len() != Self::DEFAULT_PLAYER_SIZE {
            return (0..self.slots.len()).collect();
        }
        player::HOTBAR.chain(player::MAIN).collect()
    }

    /// Anything that is not a player's inventory is a plain row of slots and is walked in order.
    fn merge_order(&self, in_hand: Option<u8>) -> Vec<usize> {
        if self.slots.len() != Self::DEFAULT_PLAYER_SIZE {
            return (0..self.slots.len()).collect();
        }
        let held = in_hand
            .filter(|slot| usize::from(*slot) < player::HOTBAR.len())
            .map(|slot| player::HOTBAR.start + usize::from(slot));
        held.into_iter()
            .chain(std::iter::once(usize::from(player::OFFHAND_SLOT)))
            .chain(player::HOTBAR)
            .chain(player::MAIN)
            .collect()
    }

    pub fn add_item_with_update(
        &mut self,
        item: InventorySlot,
        entity: Entity,
    ) -> Result<(), InventoryError> {
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(item.clone());
                INVENTORY_UPDATES_QUEUE.push(InventoryUpdate {
                    slot_index: index as u8,
                    slot: item,
                    entity,
                });
                return Ok(());
            }
        }
        Err(InventoryError::InventoryFull)
    }

    pub fn set_item(&mut self, index: usize, item: InventorySlot) -> Result<(), InventoryError> {
        if index >= self.slots.len() {
            return Err(InventoryError::InvalidSlotIndex(index));
        }
        self.slots[index] = Some(item);
        Ok(())
    }

    pub fn set_item_with_update(
        &mut self,
        index: usize,
        item: InventorySlot,
        entity: Entity,
    ) -> Result<(), InventoryError> {
        if index >= self.slots.len() {
            return Err(InventoryError::InvalidSlotIndex(index));
        }
        self.slots[index] = Some(item.clone());
        INVENTORY_UPDATES_QUEUE.push(InventoryUpdate {
            slot_index: index as u8,
            slot: item,
            entity,
        });
        Ok(())
    }

    pub fn get_item(&self, index: usize) -> Result<Option<&InventorySlot>, InventoryError> {
        if index >= self.slots.len() {
            return Err(InventoryError::InvalidSlotIndex(index));
        }
        Ok(self.slots[index].as_ref())
    }

    pub fn remove_item(&mut self, index: usize) -> Result<(), InventoryError> {
        if index >= self.slots.len() {
            return Err(InventoryError::InvalidSlotIndex(index));
        }
        if self.slots[index].is_none() {
            return Err(InventoryError::ItemNotFound);
        }
        self.slots[index] = None;
        Ok(())
    }

    pub fn remove_item_with_update(
        &mut self,
        index: usize,
        entity: Entity,
    ) -> Result<(), InventoryError> {
        if index >= self.slots.len() {
            return Err(InventoryError::InvalidSlotIndex(index));
        }
        if self.slots[index].is_none() {
            return Err(InventoryError::ItemNotFound);
        }
        self.slots[index] = None;
        INVENTORY_UPDATES_QUEUE.push(InventoryUpdate {
            slot_index: index as u8,
            slot: InventorySlot::default(),
            entity,
        });
        Ok(())
    }

    /// Clears an inventory slot, regardless of its current state, and sends an update.
    /// This is idempotent and will not error if the slot is already empty.
    pub fn clear_slot_with_update(
        &mut self,
        index: usize,
        entity: Entity,
    ) -> Result<(), InventoryError> {
        if index >= self.slots.len() {
            return Err(InventoryError::InvalidSlotIndex(index));
        }

        // If the slot is already empty, we don't need to do anything
        // except send the update (which is good practice).
        if self.slots[index].is_none() {
            // Fall through to send the update
        }

        // Set the server's state to empty
        self.slots[index] = None;

        // Queue the update to tell the client the slot is now empty
        INVENTORY_UPDATES_QUEUE.push(InventoryUpdate {
            slot_index: index as u8,
            slot: InventorySlot::default(), // An empty slot (count: 0)
            entity,
        });
        Ok(())
    }

    /// Searches the inventory for the first slot containing the given ItemID.
    ///
    /// Returns `Some(index)` if found, `None` otherwise.
    pub fn find_item(&self, item_id: ItemID) -> Option<usize> {
        self.slots.iter().position(|slot| match slot {
            Some(inventory_slot) => inventory_slot.item_id == Some(item_id),
            None => false,
        })
    }

    /// Swaps the contents of two slots and sends updates to the client.
    pub fn swap_slots_with_update(
        &mut self,
        index_a: usize,
        index_b: usize,
        entity: Entity,
    ) -> Result<(), InventoryError> {
        if index_a >= self.slots.len() {
            return Err(InventoryError::InvalidSlotIndex(index_a));
        }
        if index_b >= self.slots.len() {
            return Err(InventoryError::InvalidSlotIndex(index_b));
        }
        if index_a == index_b {
            return Ok(()); // Nothing to do
        }

        // Swap the slots in the server's memory
        self.slots.swap(index_a, index_b);

        // Send an update for the first slot
        INVENTORY_UPDATES_QUEUE.push(InventoryUpdate {
            slot_index: index_a as u8,
            // Clone the data that is now in slot A
            slot: self.slots[index_a].clone().unwrap_or_default(),
            entity,
        });

        // Send an update for the second slot
        INVENTORY_UPDATES_QUEUE.push(InventoryUpdate {
            slot_index: index_b as u8,
            // Clone the data that is now in slot B
            slot: self.slots[index_b].clone().unwrap_or_default(),
            entity,
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::ItemID;
    use ferrumc_net_codec::net_types::var_int::VarInt;

    /// A stack of one. A count of nothing is not a stack, whatever id is on it.
    fn make_slot_with_id(id: i32) -> InventorySlot {
        InventorySlot::of(ItemID(VarInt::new(id)), 1)
    }

    #[test]
    fn test_new_inventory() {
        let inv = Inventory::new(5);
        assert_eq!(inv.slots.len(), 5);
        assert!(inv.slots.iter().all(|s| s.is_none()));
    }

    #[test]
    fn test_add_and_get_item() {
        let mut inv = Inventory::new(2);
        let slot = make_slot_with_id(1);
        assert!(inv.add_item(slot.clone(), None).is_empty());
        assert!(inv.get_item(0).unwrap().is_some());
        assert!(inv.get_item(1).unwrap().is_none());
    }

    #[test]
    fn test_add_item_full() {
        let mut inv = Inventory::new(1);
        let slot = make_slot_with_id(1);
        assert!(inv.add_item(slot, None).is_empty());
        // A full inventory hands back what would not fit rather than refusing outright, so a
        // caller can put the rest back on the ground.
        let left = inv.add_item(make_slot_with_id(2), None);
        assert!(!left.is_empty());
    }

    #[test]
    fn test_set_and_remove_item() {
        let mut inv = Inventory::new(1);
        let slot = make_slot_with_id(1);
        inv.set_item(0, slot).unwrap();
        assert!(inv.get_item(0).unwrap().is_some());
        inv.remove_item(0).unwrap();
        assert!(inv.get_item(0).unwrap().is_none());
    }

    #[test]
    fn test_contains_item() {
        let mut inv = Inventory::new(2);
        let slot = make_slot_with_id(42);
        assert!(inv.add_item(slot, None).is_empty());
        assert!(inv.contains_item(42));
        assert!(!inv.contains_item(99));
    }

    #[test]
    fn test_clear() {
        let mut inv = Inventory::new(2);
        inv.set_item(0, make_slot_with_id(1)).unwrap();
        inv.set_item(1, make_slot_with_id(2)).unwrap();
        inv.clear();
        assert!(inv.slots.iter().all(|s| s.is_none()));
    }

    #[test]
    fn test_invalid_index() {
        let mut inv = Inventory::new(1);
        assert!(matches!(
            inv.get_item(2),
            Err(InventoryError::InvalidSlotIndex(2))
        ));
        assert!(matches!(
            inv.set_item(2, make_slot_with_id(1)),
            Err(InventoryError::InvalidSlotIndex(2))
        ));
        assert!(matches!(
            inv.remove_item(2),
            Err(InventoryError::InvalidSlotIndex(2))
        ));
    }
}

#[cfg(test)]
mod putting_things_away {
    use super::*;
    use crate::components::Value;
    use ferrumc_data::generated::components::ComponentType;
    use ferrumc_net_codec::net_types::var_int::VarInt;
    use ferrumc_text::ComponentBuilder;

    // The registry's own numbers, so the stack sizes below are the real ones.
    const DIRT: ItemID = ItemID(VarInt::new(55));
    const SWORD: ItemID = ItemID(VarInt::new(964));
    const PEARL: ItemID = ItemID(VarInt::new(1144));

    fn a_player() -> Inventory {
        Inventory::new(Inventory::DEFAULT_PLAYER_SIZE)
    }

    fn count_in(inventory: &Inventory, at: usize) -> i32 {
        inventory
            .get_item(at)
            .ok()
            .flatten()
            .map_or(0, |held| held.count.0)
    }

    /// The whole point: a second handful of dirt goes on the first, not into a new slot.
    #[test]
    fn a_second_handful_tops_up_the_first() {
        let mut inventory = a_player();
        assert!(
            inventory
                .add_item(InventorySlot::of(DIRT, 30), None)
                .is_empty()
        );
        assert!(
            inventory
                .add_item(InventorySlot::of(DIRT, 20), None)
                .is_empty()
        );

        let held: i32 = (0..Inventory::DEFAULT_PLAYER_SIZE)
            .map(|at| count_in(&inventory, at))
            .sum();
        assert_eq!(held, 50);
        let used = (0..Inventory::DEFAULT_PLAYER_SIZE)
            .filter(|at| count_in(&inventory, *at) > 0)
            .count();
        assert_eq!(used, 1, "one slot, not two");
    }

    #[test]
    fn what_will_not_fit_spills_into_the_next_slot() {
        let mut inventory = a_player();
        assert!(
            inventory
                .add_item(InventorySlot::of(DIRT, 100), None)
                .is_empty()
        );

        let used: Vec<i32> = player::HOTBAR
            .chain(player::MAIN)
            .map(|at| count_in(&inventory, at))
            .filter(|count| *count > 0)
            .collect();
        assert_eq!(used, vec![64, 36], "a full stack and the rest");
    }

    /// A stack's ceiling is the item's own answer, not a flat sixty-four.
    #[test]
    fn a_pearl_stacks_to_sixteen_and_a_sword_to_one() {
        assert_eq!(Inventory::stacks_to(DIRT), 64);
        assert_eq!(Inventory::stacks_to(PEARL), 16);
        assert_eq!(Inventory::stacks_to(SWORD), 1);

        let mut inventory = a_player();
        inventory.add_item(InventorySlot::of(PEARL, 20), None);
        let used: Vec<i32> = player::HOTBAR
            .chain(player::MAIN)
            .map(|at| count_in(&inventory, at))
            .filter(|count| *count > 0)
            .collect();
        assert_eq!(used, vec![16, 4]);
    }

    /// A named sword does not merge into a plain one, however alike they look.
    #[test]
    fn two_swords_that_are_not_the_same_thing_do_not_merge() {
        let mut inventory = a_player();
        let mut named = InventorySlot::of(SWORD, 1);
        named
            .components
            .set_name(&ComponentBuilder::text("Sting").build());

        inventory.add_item(named, None);
        inventory.add_item(InventorySlot::of(SWORD, 1), None);

        let used = (0..Inventory::DEFAULT_PLAYER_SIZE)
            .filter(|at| count_in(&inventory, *at) > 0)
            .count();
        assert_eq!(used, 2, "two swords, kept apart");
    }

    /// Something new never lands in the off hand, however empty it is.
    #[test]
    fn a_picked_up_block_does_not_land_in_the_shield_hand() {
        let mut inventory = a_player();
        inventory.add_item(InventorySlot::of(DIRT, 5), None);
        assert_eq!(count_in(&inventory, usize::from(player::OFFHAND_SLOT)), 0);
        assert_eq!(count_in(&inventory, player::HOTBAR.start), 5);
    }

    /// But one already held there is topped up, which is how a stack of pearls in the off hand
    /// grows.
    #[test]
    fn what_is_already_in_the_off_hand_is_topped_up() {
        let mut inventory = a_player();
        inventory
            .set_item(
                usize::from(player::OFFHAND_SLOT),
                InventorySlot::of(DIRT, 10),
            )
            .expect("the slot exists");
        inventory.add_item(InventorySlot::of(DIRT, 5), None);
        assert_eq!(count_in(&inventory, usize::from(player::OFFHAND_SLOT)), 15);
    }

    /// And two of the same damaged sword do not either, since the damage is part of what it is.
    #[test]
    fn two_stacks_with_different_damage_are_two_stacks() {
        let mut inventory = a_player();
        let mut worn = InventorySlot::of(DIRT, 1);
        worn.components.set(ComponentType::Damage, Value::Number(3));

        inventory.add_item(worn, None);
        inventory.add_item(InventorySlot::of(DIRT, 1), None);
        assert_eq!(
            (0..Inventory::DEFAULT_PLAYER_SIZE)
                .filter(|at| count_in(&inventory, *at) > 0)
                .count(),
            2
        );
    }

    /// What is being held is topped up first, which is what makes mining feel right.
    #[test]
    fn what_is_in_hand_is_filled_before_anything_else() {
        let mut inventory = a_player();
        let held = player::HOTBAR.start + 3;
        inventory
            .set_item(held, InventorySlot::of(DIRT, 10))
            .expect("the slot exists");
        inventory
            .set_item(player::MAIN.start, InventorySlot::of(DIRT, 10))
            .expect("the slot exists");

        inventory.add_item(InventorySlot::of(DIRT, 5), Some(3));
        assert_eq!(count_in(&inventory, held), 15, "the held stack grew");
        assert_eq!(count_in(&inventory, player::MAIN.start), 10, "and no other");
    }

    /// A full inventory hands back what would not fit rather than losing it.
    #[test]
    fn what_will_not_fit_comes_back() {
        let mut inventory = a_player();
        for at in player::MAIN.chain(player::HOTBAR) {
            inventory
                .set_item(at, InventorySlot::of(SWORD, 1))
                .expect("the slot exists");
        }
        let left = inventory.add_item(InventorySlot::of(DIRT, 7), None);
        assert_eq!(left.count.0, 7, "nothing fitted, and nothing was lost");
        assert_eq!(left.item_id, Some(DIRT));
    }

    #[test]
    fn a_partly_full_inventory_takes_what_it_can_and_hands_back_the_rest() {
        let mut inventory = a_player();
        for at in player::MAIN.chain(player::HOTBAR) {
            inventory
                .set_item(at, InventorySlot::of(SWORD, 1))
                .expect("the slot exists");
        }
        inventory
            .set_item(player::MAIN.start, InventorySlot::of(DIRT, 60))
            .expect("the slot exists");

        let left = inventory.add_item(InventorySlot::of(DIRT, 10), None);
        assert_eq!(count_in(&inventory, player::MAIN.start), 64);
        assert_eq!(left.count.0, 6, "and six came back");
    }

    #[test]
    fn nothing_at_all_goes_nowhere() {
        let mut inventory = a_player();
        assert!(inventory.add_item(InventorySlot::empty(), None).is_empty());
    }
}
