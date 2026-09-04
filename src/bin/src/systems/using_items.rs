//! Right-clicking and holding.
//!
//! A right-click on nothing starts a use; the use counts down while it is held; something happens
//! when it reaches the end. Eating and drinking are what is here. A bow, a crossbow and a trident
//! are held down the same way and each needs what happens at the end of it, which is a projectile.
//!
//! Two flags in the entity's data are what makes the client draw the arm and the progress: one says
//! something is being used, the other says which hand. Both have to go out when the use starts and
//! come off when it stops, or a client goes on animating an item that was finished with.

use bevy_ecs::prelude::*;
use ferrumc_core::identity::player_identity::PlayerIdentity;
use ferrumc_data::generated::items::{DataComponent, Item, UseRemainderImpl};
use ferrumc_entities::synced_data::{LivingFlag, SyncedData};
use ferrumc_inventories::hotbar::Hotbar;
use ferrumc_inventories::inventory::Inventory;
use ferrumc_inventories::item::ItemID;
use ferrumc_inventories::slot::InventorySlot;
use ferrumc_inventories::using::{how_long, Hand, UsingItem};
use ferrumc_messages::PlayerEating;
use ferrumc_net::packets::incoming::use_item::Hand as WireHand;
use ferrumc_net::UseItemReceiver;
use ferrumc_net_codec::net_types::var_int::VarInt;
use tracing::warn;

/// What is read off a player to start a use.
type Reaching<'a> = (&'a Inventory, &'a Hotbar, &'a mut SyncedData);

/// Starts holding something down, where what is held can be.
pub fn start_using(
    used: Res<UseItemReceiver>,
    mut players: Query<Reaching, With<PlayerIdentity>>,
    already: Query<&UsingItem>,
    mut commands: Commands,
) {
    for (packet, player) in used.0.try_iter() {
        // A second right-click while already eating changes nothing, which is what stops a held
        // button starting the meal over every tick.
        if already.get(player).is_ok() {
            continue;
        }
        let Ok((inventory, hotbar, mut data)) = players.get_mut(player) else {
            continue;
        };

        let (hand, slot) = match packet.hand {
            WireHand::MainHand => (Hand::Main, hotbar.get_selected_inventory_index()),
            WireHand::OffHand => (Hand::Off, Inventory::OFFHAND_SLOT),
        };
        let Some(item) = inventory
            .get_item(slot)
            .ok()
            .flatten()
            .filter(|held| !held.is_empty())
            .and_then(|held| held.item_id)
            .and_then(|id| u16::try_from(id.0 .0).ok())
        else {
            continue;
        };
        let Some(takes) = how_long(item) else {
            // Not something that can be held down. Placing a block and throwing a pearl are both
            // right-clicks too and neither is this.
            continue;
        };

        commands.entity(player).insert(UsingItem {
            hand,
            slot,
            item,
            left: takes,
            takes,
        });
        // What makes the client raise the arm and draw the bar.
        data.set_living_flag(LivingFlag::UsingItem, true);
        data.set_living_flag(LivingFlag::OffHand, hand == Hand::Off);
    }
}

/// What is read off someone in the middle of using something.
type Using<'a> = (
    Entity,
    &'a mut UsingItem,
    &'a mut Inventory,
    &'a mut SyncedData,
);

/// Counts a use down, and finishes it.
pub fn tick_using(
    mut users: Query<Using>,
    mut eaten: MessageWriter<PlayerEating>,
    mut commands: Commands,
) {
    for (player, mut using, mut inventory, mut data) in &mut users {
        // Putting the item down stops the use. Without this, swapping to a sword mid-meal would
        // still finish the meal.
        let still_held = inventory
            .get_item(using.slot)
            .ok()
            .flatten()
            .and_then(|held| held.item_id)
            .and_then(|id| u16::try_from(id.0 .0).ok())
            == Some(using.item);
        if !still_held {
            stop(&mut commands, player, &mut data);
            continue;
        }

        if !using.tick() {
            continue;
        }

        // Finished. One is spent, and whatever it leaves behind takes its place where the slot is
        // now empty — an empty bottle after a potion.
        eaten.write(PlayerEating {
            player,
            item: ItemID(VarInt::new(i32::from(using.item))),
        });
        spend_one(&mut inventory, using.slot, using.item);
        stop(&mut commands, player, &mut data);
    }
}

/// Stops a use and tells the client it has stopped.
fn stop(commands: &mut Commands, player: Entity, data: &mut SyncedData) {
    commands.entity(player).remove::<UsingItem>();
    data.set_living_flag(LivingFlag::UsingItem, false);
    data.set_living_flag(LivingFlag::OffHand, false);
}

/// Takes one off a stack, and puts what it leaves behind in its place.
fn spend_one(inventory: &mut Inventory, slot: usize, item: u16) {
    let Ok(Some(mut held)) = inventory.get_item(slot).map(|held| held.cloned()) else {
        return;
    };
    held.count.0 -= 1;

    let left = Item::from_id(item)
        .and_then(|item| {
            item.components.iter().find_map(|(id, data)| {
                (*id == DataComponent::UseRemainder)
                    .then(|| data.as_any().downcast_ref::<UseRemainderImpl>())
                    .flatten()
            })
        })
        .map(|left| InventorySlot::of(ItemID(VarInt::new(i32::from(left.item))), 1));

    let put = match (held.count.0 > 0, left) {
        // Still more of it, so what it leaves behind has to go somewhere else.
        (true, Some(left)) => {
            let _ = inventory.set_item(slot, held);
            let over = inventory.add_item(left, None);
            if !over.is_empty() {
                warn!("an empty bottle had nowhere to go and was lost");
            }
            return;
        }
        (true, None) => held,
        (false, Some(left)) => left,
        (false, None) => InventorySlot::empty(),
    };
    let _ = inventory.set_item(slot, put);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::message::{MessageRegistry, Messages};
    use bevy_ecs::schedule::Schedule;
    use ferrumc_entities::entity_type::EntityType;

    fn id(name: &str) -> u16 {
        Item::from_registry_key(name).expect("it is an item").id
    }

    /// A world holding one player already partway through a meal.
    fn a_world_eating(what: &str, count: i32) -> (World, Entity, Schedule) {
        let mut world = World::new();
        MessageRegistry::register_message::<PlayerEating>(&mut world);

        let slot = usize::from(ferrumc_inventories::defined_slots::player::HOTBAR_SLOT_1);
        let item = id(what);
        let mut inventory = Inventory::new(Inventory::DEFAULT_PLAYER_SIZE);
        inventory
            .set_item(
                slot,
                InventorySlot::of(ItemID(VarInt::new(i32::from(item))), count),
            )
            .expect("the slot exists");

        let player = world
            .spawn((
                inventory,
                Hotbar::default(),
                SyncedData::new(EntityType::Player),
                UsingItem {
                    hand: Hand::Main,
                    slot,
                    item,
                    left: 1,
                    takes: 32,
                },
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(tick_using);
        (world, player, schedule)
    }

    fn slot_of(world: &World, player: Entity, at: usize) -> Option<InventorySlot> {
        world
            .get::<Inventory>(player)
            .expect("it has an inventory")
            .get_item(at)
            .ok()
            .flatten()
            .cloned()
    }

    /// Finishing a meal says so, and spends one.
    #[test]
    fn finishing_a_meal_spends_one_and_says_what_was_eaten() {
        let (mut world, player, mut schedule) = a_world_eating("minecraft:cooked_beef", 3);
        let slot = usize::from(ferrumc_inventories::defined_slots::player::HOTBAR_SLOT_1);

        schedule.run(&mut world);
        assert!(!world.resource::<Messages<PlayerEating>>().is_empty());
        assert_eq!(slot_of(&world, player, slot).expect("some left").count.0, 2);
        assert!(
            world.get::<UsingItem>(player).is_none(),
            "and the use is over"
        );
    }

    /// Drinking a potion leaves the bottle in its place.
    #[test]
    fn drinking_the_last_potion_leaves_the_bottle() {
        let (mut world, player, mut schedule) = a_world_eating("minecraft:potion", 1);
        let slot = usize::from(ferrumc_inventories::defined_slots::player::HOTBAR_SLOT_1);

        schedule.run(&mut world);
        let left = slot_of(&world, player, slot).expect("something is there");
        assert_eq!(
            left.item_id,
            Some(ItemID(VarInt::new(i32::from(id("minecraft:glass_bottle")))))
        );
    }

    /// And where there are more potions, the bottle goes somewhere else rather than replacing them.
    #[test]
    fn drinking_one_of_several_puts_the_bottle_elsewhere() {
        let (mut world, player, mut schedule) = a_world_eating("minecraft:potion", 2);
        let slot = usize::from(ferrumc_inventories::defined_slots::player::HOTBAR_SLOT_1);

        schedule.run(&mut world);
        assert_eq!(
            slot_of(&world, player, slot).expect("some left").item_id,
            Some(ItemID(VarInt::new(i32::from(id("minecraft:potion")))))
        );
        let bottle = ItemID(VarInt::new(i32::from(id("minecraft:glass_bottle"))));
        assert!(
            world
                .get::<Inventory>(player)
                .expect("an inventory")
                .contains_item(bottle.0 .0),
            "the bottle went somewhere"
        );
    }

    /// Putting the item down stops the meal rather than finishing it.
    #[test]
    fn swapping_the_item_away_stops_the_use() {
        let (mut world, player, mut schedule) = a_world_eating("minecraft:cooked_beef", 1);
        let slot = usize::from(ferrumc_inventories::defined_slots::player::HOTBAR_SLOT_1);
        world
            .get_mut::<Inventory>(player)
            .expect("an inventory")
            .set_item(slot, InventorySlot::empty())
            .expect("the slot exists");

        schedule.run(&mut world);
        assert!(world.resource::<Messages<PlayerEating>>().is_empty());
        assert!(world.get::<UsingItem>(player).is_none());
    }
}
