//! Keeping an entity's numbers in step with what it is wearing, and telling clients about them.
//!
//! An attribute is a base value plus a stack of modifiers, and almost every modifier comes from
//! something being worn or held. Nothing writes an attribute directly: putting on a helmet adds a
//! modifier named after the helmet, taking it off removes that name, and the base is never touched
//! — which is what makes taking it off exact rather than nearly right.
//!
//! A client is told the base and the modifiers separately rather than the total, because that is
//! what lets it draw the attack cooldown bar and show where a number came from.

use bevy_ecs::prelude::*;
use ferrumc_attributes::{Attributes, Modifier, Operation};
use ferrumc_components::health::Health;
use ferrumc_core::identity::entity_identity::EntityIdentity;
use ferrumc_core::identity::player_identity::PlayerIdentity;
use ferrumc_data::attributes::Attribute;
use ferrumc_data::generated::enchantments::{Hook as EnchantHook, Operation as EnchantOperation};
use ferrumc_data::generated::items::{
    AttributeModifierSlot, Item, Modifier as ItemModifier, Operation as ItemOperation,
};
use ferrumc_inventories::hotbar::Hotbar;
use ferrumc_inventories::inventory::Inventory;
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::update_attributes::{
    Snapshot, UpdateAttributesPacket, WireModifier,
};
use ferrumc_net_codec::registry_remap::NetworkAttribute;
use tracing::warn;

/// Which slot a modifier came from, prefixed onto its name.
///
/// An item names its own modifiers, and vanilla's names already differ per piece of armour — but
/// nothing stops the same item being held in both hands, and two modifiers under one name are one
/// modifier. The slot in front keeps them apart.
fn named_for(slot: &str, modifier: &ItemModifier) -> String {
    format!("{slot}/{}", modifier.id)
}

/// What an entity is wearing and holding, as far as its numbers are concerned.
type Wearing<'a> = (&'a mut Attributes, &'a Inventory, &'a Hotbar);

/// When it is worth looking again: something was picked up, put down, or has only just appeared.
///
/// Without this the whole set would be rewritten every tick, and since writing to it is what marks
/// it changed, every client would be sent every entity's numbers twenty times a second.
type SomethingChangedHands = Or<(Changed<Inventory>, Changed<Hotbar>, Added<Attributes>)>;

/// Everything one slot changes about the wearer's numbers: what the item is worth, and what it is
/// enchanted with.
///
/// An enchantment is not a special case — efficiency, respiration and aqua affinity are attribute
/// modifiers like any other, and the packs say which attribute and by how much.
fn from_one_slot(
    attributes: &mut Attributes,
    slot: &str,
    held: Option<&'static Item>,
    stack: Option<&ferrumc_inventories::components::Components>,
) {
    let Some(item) = held else {
        return;
    };

    for modifier in item.attribute_modifiers() {
        if !fits(modifier, slot) {
            continue;
        }
        let Some(attribute) = Attribute::from_name(modifier.r#type.name) else {
            continue;
        };
        attributes.add(
            attribute,
            Modifier {
                name: named_for(slot, modifier).into(),
                amount: modifier.amount,
                operation: operation_of(&modifier.operation),
            },
        );
    }

    let Some(stack) = stack else { return };
    for (enchantment, level) in stack.enchantments() {
        for effect in enchantment.effects {
            let EnchantHook::Attribute {
                attribute,
                name,
                operation,
            } = effect.hook
            else {
                continue;
            };
            let Some(attribute) = Attribute::from_name(attribute) else {
                continue;
            };
            attributes.add(
                attribute,
                Modifier {
                    name: format!("{slot}/{name}").into(),
                    amount: f64::from(effect.value.at(level)),
                    operation: match operation {
                        EnchantOperation::AddValue => Operation::AddValue,
                        EnchantOperation::AddMultipliedBase => Operation::AddMultipliedBase,
                        EnchantOperation::AddMultipliedTotal => Operation::AddMultipliedTotal,
                    },
                },
            );
        }
    }
}

/// Puts what is worn and held onto the numbers, and takes off what is no longer there.
pub fn apply_what_is_worn(mut wearers: Query<Wearing, SomethingChangedHands>) {
    for (mut attributes, inventory, hotbar) in &mut wearers {
        if attributes.is_empty() {
            continue;
        }

        // Every slot that can change a number, and what is in it. A slot is visited whether or not
        // it holds anything, so emptying one takes its modifiers away.
        let mut worn: Vec<(&'static str, usize)> =
            Vec::with_capacity(Inventory::ARMOUR_SLOTS.len() + 2);
        for (slot, name) in Inventory::ARMOUR_SLOTS
            .iter()
            .zip(Inventory::ARMOUR_SLOT_NAMES)
        {
            worn.push((name, *slot));
        }
        worn.push(("mainhand", hotbar.get_selected_inventory_index()));
        worn.push(("offhand", Inventory::OFFHAND_SLOT));

        for (slot, at) in worn {
            // Whatever this slot used to say is dropped first, so a swapped piece does not leave
            // half of the old one behind.
            attributes.remove_by_prefix(&format!("{slot}/"));

            let stack = inventory.get_item(at).ok().flatten();
            from_one_slot(
                &mut attributes,
                slot,
                item_in(inventory, at),
                stack.map(|held| &held.components),
            );
        }
    }
}

/// What is read off an entity to say what its numbers are, and by what name a client knows it.
type Numbers<'a> = (
    &'a Attributes,
    Option<&'a EntityIdentity>,
    Option<&'a PlayerIdentity>,
);

/// Tells clients an entity's numbers whenever they have changed.
pub fn send_changed_attributes(
    changed: Query<Numbers, Changed<Attributes>>,
    watchers: Query<&StreamWriter>,
) {
    for (attributes, identity, player) in &changed {
        let id = identity
            .map(|identity| identity.entity_id)
            .or_else(|| player.map(|player| player.short_uuid));
        let Some(id) = id else { continue };

        let values: Vec<Snapshot> = attributes
            .iter()
            .map(|(attribute, instance)| Snapshot {
                attribute: NetworkAttribute(u32::from(attribute.id)),
                base: instance.base(),
                modifiers: instance
                    .modifiers()
                    .map(|modifier| WireModifier {
                        // A client tells modifiers apart by name, and a name it cannot read is a
                        // disconnect, so what goes out is always a resource location.
                        name: as_a_resource_location(&modifier.name),
                        amount: modifier.amount,
                        operation: modifier.operation as u8,
                    })
                    .collect(),
            })
            .collect();

        let packet = UpdateAttributesPacket::new(id, values);
        if packet.is_empty() {
            continue;
        }
        for writer in &watchers {
            if let Err(err) = writer.send_packet_ref(&packet) {
                warn!("could not tell a player an entity's numbers: {err:?}");
            }
        }
    }
}

/// What is in one slot, if it is something the game knows.
fn item_in(inventory: &Inventory, slot: usize) -> Option<&'static Item> {
    let held = inventory.get_item(slot).ok().flatten()?;
    let id = held.item_id?;
    Item::from_id(u16::try_from(id.0 .0).ok()?)
}

/// Whether a modifier applies in the slot the item is in.
fn fits(modifier: &ItemModifier, slot: &str) -> bool {
    match modifier.slot {
        AttributeModifierSlot::Any => true,
        AttributeModifierSlot::String(where_it_works) => match where_it_works {
            // The three groups an item may name instead of one slot.
            "any" => true,
            "armor" => Inventory::ARMOUR_SLOT_NAMES.contains(&slot),
            "hand" => slot == "mainhand" || slot == "offhand",
            named => named == slot,
        },
    }
}

/// The same operation, said in the attribute system's own terms.
const fn operation_of(operation: &ItemOperation) -> Operation {
    match operation {
        ItemOperation::AddValue => Operation::AddValue,
        ItemOperation::AddMultipliedBase => Operation::AddMultipliedBase,
        ItemOperation::AddMultipliedTotal => Operation::AddMultipliedTotal,
    }
}

/// A modifier name a client will accept.
///
/// The names here carry the slot in front of them, which is not a resource location; a client reads
/// one and disconnects if it cannot. The slot is folded into the path rather than dropped, so two
/// hands holding the same thing stay two modifiers.
fn as_a_resource_location(name: &str) -> String {
    let Some((slot, rest)) = name.split_once('/') else {
        return name.to_string();
    };
    let (namespace, path) = rest.split_once(':').unwrap_or(("minecraft", rest));
    format!("{namespace}:{path}.{slot}")
}

/// Keeps how much health a thing can have in step with what its numbers say.
///
/// Health is its own component because that is what the rest of the server reads, but what the
/// ceiling is belongs to the attributes: a modifier that raises it raises the ceiling, and one that
/// lowers it below where the thing already is brings it down to the new ceiling.
pub fn follow_max_health(mut living: Query<(&Attributes, &mut Health), Changed<Attributes>>) {
    for (attributes, mut health) in &mut living {
        let Some(max) = attributes.get(&Attribute::MAX_HEALTH) else {
            continue;
        };
        let max = max.value() as f32;
        if (health.max - max).abs() < f32::EPSILON {
            continue;
        }
        health.max = max;
        health.current = health.current.min(max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::Schedule;
    use ferrumc_data::generated::components::ComponentType;
    use ferrumc_entities::entity_type::EntityType;
    use ferrumc_inventories::components::Value;
    use ferrumc_inventories::item::ItemID;
    use ferrumc_inventories::slot::InventorySlot;
    use ferrumc_net_codec::net_types::var_int::VarInt;

    /// A world holding one player, wearing nothing.
    fn a_player() -> (World, Entity, Schedule) {
        let mut world = World::new();
        let player = world
            .spawn((
                Attributes::for_entity(EntityType::Player.protocol_id()),
                Inventory::new(Inventory::DEFAULT_PLAYER_SIZE),
                Hotbar::default(),
                Health::default(),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems((apply_what_is_worn, follow_max_health).chain());
        schedule.run(&mut world);
        (world, player, schedule)
    }

    fn wear(world: &mut World, player: Entity, slot: usize, item: &str) {
        let id = Item::from_registry_key(item).expect("it is an item").id;
        let mut inventory = world
            .get_mut::<Inventory>(player)
            .expect("a player has an inventory");
        inventory
            .set_item(
                slot,
                InventorySlot {
                    item_id: Some(ItemID(VarInt::new(i32::from(id)))),
                    count: VarInt::new(1),
                    ..Default::default()
                },
            )
            .expect("the slot exists");
    }

    fn armour(world: &World, player: Entity) -> f64 {
        world
            .get::<Attributes>(player)
            .expect("a player has numbers")
            .value(&Attribute::ARMOR)
    }

    #[test]
    fn putting_on_a_helmet_raises_the_armour_and_taking_it_off_puts_it_back() {
        let (mut world, player, mut schedule) = a_player();
        assert_eq!(armour(&world, player), 0.0);

        wear(
            &mut world,
            player,
            Inventory::ARMOUR_SLOTS[0],
            "minecraft:diamond_helmet",
        );
        schedule.run(&mut world);
        assert_eq!(armour(&world, player), 3.0, "what the helmet is worth");

        let mut inventory = world
            .get_mut::<Inventory>(player)
            .expect("a player has an inventory");
        inventory
            .set_item(Inventory::ARMOUR_SLOTS[0], InventorySlot::empty())
            .expect("the slot exists");
        schedule.run(&mut world);
        assert_eq!(
            armour(&world, player),
            0.0,
            "exactly back, not nearly: the base was never touched"
        );
    }

    #[test]
    fn a_full_set_adds_up() {
        let (mut world, player, mut schedule) = a_player();
        for (slot, piece) in Inventory::ARMOUR_SLOTS.iter().zip([
            "minecraft:diamond_helmet",
            "minecraft:diamond_chestplate",
            "minecraft:diamond_leggings",
            "minecraft:diamond_boots",
        ]) {
            wear(&mut world, player, *slot, piece);
        }
        schedule.run(&mut world);
        assert_eq!(armour(&world, player), 20.0, "three, eight, six and three");
    }

    /// A helmet's modifier says `head`, so it does nothing anywhere else.
    #[test]
    fn a_helmet_held_in_a_hand_is_not_armour() {
        let (mut world, player, mut schedule) = a_player();
        wear(
            &mut world,
            player,
            Inventory::OFFHAND_SLOT,
            "minecraft:diamond_helmet",
        );
        schedule.run(&mut world);
        assert_eq!(armour(&world, player), 0.0);
    }

    /// Efficiency is an attribute modifier like any other, and the packs say so — level five is
    /// the level squared plus one, which is why it runs away.
    #[test]
    fn an_enchantment_moves_a_number_the_same_way_an_item_does() {
        let (mut world, player, mut schedule) = a_player();
        let hand = world
            .get::<Hotbar>(player)
            .expect("a player has a hotbar")
            .get_selected_inventory_index();

        let id = Item::from_registry_key("minecraft:diamond_pickaxe")
            .expect("it is an item")
            .id;
        let efficiency =
            ferrumc_data::generated::enchantments::Enchantment::from_name("efficiency")
                .expect("it is an enchantment");
        let mut pickaxe = InventorySlot {
            item_id: Some(ItemID(VarInt::new(i32::from(id)))),
            count: VarInt::new(1),
            ..Default::default()
        };
        pickaxe.components.set(
            ComponentType::Enchantments,
            Value::Enchantments(vec![(efficiency.id, 5)]),
        );
        world
            .get_mut::<Inventory>(player)
            .expect("a player has an inventory")
            .set_item(hand, pickaxe)
            .expect("the slot exists");

        schedule.run(&mut world);
        let numbers = world
            .get::<Attributes>(player)
            .expect("a player has numbers");
        assert_eq!(
            numbers.value(&Attribute::MINING_EFFICIENCY),
            26.0,
            "five squared and one"
        );
    }

    /// And taking it off puts the number back exactly.
    #[test]
    fn taking_the_enchanted_tool_off_puts_the_number_back() {
        let (mut world, player, mut schedule) = a_player();
        let hand = world
            .get::<Hotbar>(player)
            .expect("a player has a hotbar")
            .get_selected_inventory_index();

        let id = Item::from_registry_key("minecraft:diamond_pickaxe")
            .expect("it is an item")
            .id;
        let efficiency =
            ferrumc_data::generated::enchantments::Enchantment::from_name("efficiency")
                .expect("it is an enchantment");
        let mut pickaxe = InventorySlot {
            item_id: Some(ItemID(VarInt::new(i32::from(id)))),
            count: VarInt::new(1),
            ..Default::default()
        };
        pickaxe.components.set(
            ComponentType::Enchantments,
            Value::Enchantments(vec![(efficiency.id, 3)]),
        );
        world
            .get_mut::<Inventory>(player)
            .expect("a player has an inventory")
            .set_item(hand, pickaxe)
            .expect("the slot exists");
        schedule.run(&mut world);
        assert!(
            world
                .get::<Attributes>(player)
                .expect("numbers")
                .value(&Attribute::MINING_EFFICIENCY)
                > 0.0
        );

        world
            .get_mut::<Inventory>(player)
            .expect("a player has an inventory")
            .set_item(hand, InventorySlot::empty())
            .expect("the slot exists");
        schedule.run(&mut world);
        assert_eq!(
            world
                .get::<Attributes>(player)
                .expect("numbers")
                .value(&Attribute::MINING_EFFICIENCY),
            0.0,
            "exactly back"
        );
    }

    /// A weapon changes what the holder hits for, which is the same machinery.
    #[test]
    fn a_sword_in_the_hand_changes_what_the_holder_hits_for() {
        let (mut world, player, mut schedule) = a_player();
        let hand = world
            .get::<Hotbar>(player)
            .expect("a player has a hotbar")
            .get_selected_inventory_index();

        wear(&mut world, player, hand, "minecraft:diamond_sword");
        schedule.run(&mut world);

        let numbers = world
            .get::<Attributes>(player)
            .expect("a player has numbers");
        assert_eq!(numbers.value(&Attribute::ATTACK_DAMAGE), 7.0);
        assert!((numbers.value(&Attribute::ATTACK_SPEED) - 1.6).abs() < 1e-6);
    }

    #[test]
    fn a_slot_name_becomes_part_of_the_path_rather_than_breaking_it() {
        assert_eq!(
            as_a_resource_location("head/minecraft:armor.helmet"),
            "minecraft:armor.helmet.head"
        );
        assert_eq!(
            as_a_resource_location("minecraft:already_fine"),
            "minecraft:already_fine"
        );
    }

    #[test]
    fn an_item_that_works_in_a_group_works_in_every_slot_of_it() {
        let boots = ItemModifier {
            r#type: &Attribute::ARMOR,
            id: "minecraft:armor.boots",
            amount: 3.0,
            operation: ItemOperation::AddValue,
            slot: AttributeModifierSlot::String("armor"),
        };
        assert!(fits(&boots, "feet"));
        assert!(fits(&boots, "head"));
        assert!(!fits(&boots, "mainhand"));

        let sword = ItemModifier {
            slot: AttributeModifierSlot::String("mainhand"),
            ..boots.clone()
        };
        assert!(fits(&sword, "mainhand"));
        assert!(!fits(&sword, "offhand"));
    }
}
