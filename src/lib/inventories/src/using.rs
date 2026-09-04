//! Holding an item down.
//!
//! Eating, drinking and drawing a bow are all the same shape: a right-click starts it, it counts
//! down while the button is held, and something happens when it reaches the end. Which of those a
//! particular item does is on the item — a consumable takes as long as its `consume_seconds` says,
//! and everything else is not something that can be held down at all.
//!
//! A client draws the progress bar itself from two flags in the entity's data, which is why the
//! server has to say when one starts and when it stops rather than only when it finishes.

use bevy_ecs::prelude::Component;
use ferrumc_data::generated::items::{ConsumableImpl, DataComponent, Item};

/// How long a consumable takes where it says nothing else, in seconds.
///
/// A hair over a second and a half, which is what almost everything eats in.
pub const DEFAULT_CONSUME_SECONDS: f32 = 1.6;

/// How many ticks a second, which is what a duration is counted in.
const TICKS_A_SECOND: f32 = 20.0;

/// Which hand something is being used in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hand {
    Main,
    Off,
}

/// An item being held down.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct UsingItem {
    pub hand: Hand,
    /// Which slot it is in, so putting it down stops the use.
    pub slot: usize,
    /// What it is, so swapping to another of the same kind does not go on eating the first.
    pub item: u16,
    /// Ticks still to go.
    pub left: u16,
    /// How long it takes in total, which is what the progress bar is drawn from.
    pub takes: u16,
}

impl UsingItem {
    /// One tick passing. Returns whether it has just finished.
    pub const fn tick(&mut self) -> bool {
        self.left = self.left.saturating_sub(1);
        self.left == 0
    }

    /// How far through it is, from nothing to one.
    #[must_use]
    pub fn progress(&self) -> f32 {
        if self.takes == 0 {
            return 1.0;
        }
        1.0 - f32::from(self.left) / f32::from(self.takes)
    }
}

/// How long an item takes to use, or nothing where it is not something that can be held down.
///
/// Only a consumable so far. A bow, a crossbow and a trident are all held down too, and each needs
/// what happens at the end of it before the holding is worth anything.
#[must_use]
pub fn how_long(item: u16) -> Option<u16> {
    let item = Item::from_id(item)?;
    let consumable = item.components.iter().find_map(|(id, data)| {
        (*id == DataComponent::Consumable)
            .then(|| data.as_any().downcast_ref::<ConsumableImpl>())
            .flatten()
    })?;
    let ticks = (consumable.consume_seconds * TICKS_A_SECOND).ceil();
    Some(ticks.max(0.0) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(name: &str) -> u16 {
        Item::from_registry_key(name).expect("it is an item").id
    }

    /// Almost everything eats in a hair over a second and a half.
    #[test]
    fn a_steak_takes_thirty_two_ticks() {
        assert_eq!(how_long(id("minecraft:cooked_beef")), Some(32));
    }

    /// And a few take their own time.
    #[test]
    fn honey_takes_longer_and_a_bottle_of_potion_takes_the_usual() {
        assert_eq!(how_long(id("minecraft:honey_bottle")), Some(40));
        assert_eq!(how_long(id("minecraft:potion")), Some(32));
    }

    #[test]
    fn something_that_cannot_be_held_down_takes_no_time_at_all() {
        assert_eq!(how_long(id("minecraft:dirt")), None);
        assert_eq!(how_long(id("minecraft:diamond_sword")), None);
    }

    #[test]
    fn a_use_counts_down_and_says_when_it_is_done() {
        let mut using = UsingItem {
            hand: Hand::Main,
            slot: 36,
            item: id("minecraft:cooked_beef"),
            left: 3,
            takes: 3,
        };
        assert!(!using.tick());
        assert!(!using.tick());
        assert!(using.tick(), "the third tick finishes it");
    }

    #[test]
    fn the_progress_bar_runs_from_nothing_to_one() {
        let mut using = UsingItem {
            hand: Hand::Main,
            slot: 36,
            item: id("minecraft:cooked_beef"),
            left: 4,
            takes: 4,
        };
        assert_eq!(using.progress(), 0.0);
        using.tick();
        using.tick();
        assert_eq!(using.progress(), 0.5);
    }
}
