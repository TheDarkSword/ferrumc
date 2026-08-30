//! What tells an advancement that something happened.
//!
//! A criterion names a trigger and the conditions the event has to meet. Gameplay fires a trigger,
//! every criterion listening for it is offered the event, and the ones whose conditions hold are
//! granted.
//!
//! Most of the game's triggers need something that does not exist here yet. Those are read, so an
//! advancement carrying one still loads and shows on the screen, and they never fire.

use crate::AdvancementError;
use ferrumc_predicates::{Bounds, ItemPredicate};
use serde_json::Value;

/// The event a criterion is waiting for.
#[derive(Clone, Debug)]
pub enum Trigger {
    /// Never happens. Vanilla uses it for the roots of trees that are shown but never earned.
    Impossible,
    /// Every tick, for a criterion that only wants the player to exist.
    Tick,
    /// What the player is carrying changed.
    InventoryChanged {
        items: Vec<ItemPredicate>,
        slots_occupied: Bounds,
        slots_full: Bounds,
        slots_empty: Bounds,
    },
    /// One whose gameplay does not exist yet. Never fires.
    NotYet(&'static str),
}

/// One named condition of an advancement.
#[derive(Clone, Debug)]
pub struct Criterion {
    pub trigger: Trigger,
}

impl Criterion {
    pub fn parse(name: &str, value: &Value) -> Result<Self, AdvancementError> {
        let trigger = value
            .get("trigger")
            .and_then(Value::as_str)
            .ok_or_else(|| AdvancementError::NoTrigger(name.to_owned()))?;
        let bare = trigger.strip_prefix("minecraft:").unwrap_or(trigger);
        let conditions = value.get("conditions");

        let trigger = match bare {
            "impossible" => Trigger::Impossible,
            "tick" => Trigger::Tick,
            "inventory_changed" => {
                let slots = conditions.and_then(|c| c.get("slots"));
                let bound =
                    |name: &str| slots.map_or(Bounds::ANY, |slots| Bounds::field(slots, name));
                Trigger::InventoryChanged {
                    items: conditions
                        .and_then(|c| c.get("items"))
                        .and_then(Value::as_array)
                        .map(|items| items.iter().filter_map(ItemPredicate::parse).collect())
                        .unwrap_or_default(),
                    slots_occupied: bound("occupied"),
                    slots_full: bound("full"),
                    slots_empty: bound("empty"),
                }
            }
            other if KNOWN.contains(&other) => Trigger::NotYet(
                KNOWN
                    .iter()
                    .find(|known| **known == other)
                    .copied()
                    .unwrap_or("unknown"),
            ),
            _ => return Err(AdvancementError::UnknownTrigger(trigger.to_owned())),
        };
        Ok(Self { trigger })
    }
}

/// What a player is carrying, as much of it as a criterion reads.
pub struct Carried<'a> {
    /// Every slot, `None` where it is empty, as an item's registry id and how many.
    pub slots: &'a [Option<(i32, i32)>],
}

impl Carried<'_> {
    fn occupied(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    fn empty(&self) -> usize {
        self.slots.len() - self.occupied()
    }
}

impl Trigger {
    /// Whether a change to what the player carries meets this criterion.
    ///
    /// Every item the criterion names has to be somewhere in the inventory, and the slot counts
    /// have to be within their bounds.
    #[must_use]
    pub fn inventory_meets(&self, carried: &Carried) -> bool {
        let Self::InventoryChanged {
            items,
            slots_occupied,
            slots_full,
            slots_empty,
        } = self
        else {
            return false;
        };
        let tags = ferrumc_registry::tags::current().item();
        // Vanilla counts a slot full at the item's own maximum stack size, which nothing carries
        // yet; until it does, only a stack of the largest a slot holds counts.
        let full = carried
            .slots
            .iter()
            .flatten()
            .filter(|(_, count)| *count >= 64)
            .count();

        slots_occupied.matches(carried.occupied() as f64)
            && slots_empty.matches(carried.empty() as f64)
            && slots_full.matches(full as f64)
            && items.iter().all(|wanted| {
                carried.slots.iter().flatten().any(|(item, count)| {
                    wanted.matches(
                        &tags,
                        ferrumc_predicates::context::ItemRef {
                            id: *item,
                            count: *count,
                        },
                    )
                })
            })
    }
}

/// Every trigger the game has, so one that does not fire is still told apart from one that does
/// not exist.
const KNOWN: &[&str] = &[
    "player_killed_entity",
    "entity_killed_player",
    "enter_block",
    "recipe_unlocked",
    "player_hurt_entity",
    "entity_hurt_player",
    "enchanted_item",
    "filled_bucket",
    "brewed_potion",
    "construct_beacon",
    "used_ender_eye",
    "summoned_entity",
    "bred_animals",
    "location",
    "slept_in_bed",
    "cured_zombie_villager",
    "villager_trade",
    "item_durability_changed",
    "levitation",
    "changed_dimension",
    "tame_animal",
    "placed_block",
    "consume_item",
    "effects_changed",
    "used_totem",
    "nether_travel",
    "fishing_rod_hooked",
    "channeled_lightning",
    "shot_crossbow",
    "spear_mobs",
    "killed_by_arrow",
    "hero_of_the_village",
    "voluntary_exile",
    "slide_down_block",
    "bee_nest_destroyed",
    "target_hit",
    "item_used_on_block",
    "default_block_use",
    "any_block_use",
    "player_generates_container_loot",
    "thrown_item_picked_up_by_entity",
    "thrown_item_picked_up_by_player",
    "player_interacted_with_entity",
    "player_sheared_equipment",
    "started_riding",
    "lightning_strike",
    "using_item",
    "fall_from_height",
    "ride_entity_in_lava",
    "kill_mob_near_sculk_catalyst",
    "allay_drop_item_on_block",
    "avoid_vibration",
    "recipe_crafted",
    "crafter_recipe_crafted",
    "fall_after_explosion",
];
