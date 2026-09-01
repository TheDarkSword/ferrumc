//! What is left lying on the ground: dropped items and experience.
//!
//! Both behave the same way in outline — they appear, they wait, they are picked up or they give
//! up and vanish — and both merge with their neighbours so a floor covered in cobblestone is a
//! handful of entities rather than a thousand.
//!
//! Nothing here touches the world. What is here is the arithmetic: how long a thing waits, what an
//! amount of experience breaks into, and how hard an orb is pulled towards whoever is nearest.

use bevy_ecs::prelude::Component;
use bevy_math::{DVec3, Vec3A};
use ferrumc_inventories::slot::InventorySlot;

/// How long a dropped thing lies there before it gives up, in ticks.
///
/// Five minutes, for both items and experience.
pub const LIFETIME: u32 = 6000;

/// How long before anyone may pick up something that was just dropped.
pub const PICKUP_DELAY: u16 = 10;

/// How far apart two dropped things may be and still become one, in blocks.
pub const MERGE_REACH: f64 = 0.5;

/// How often a dropped thing looks for a neighbour to join, in ticks.
///
/// Vanilla looks every other tick while one is moving and every fortieth once it has settled; the
/// slower of the two is what matters, since a floor of dropped stone is settled.
pub const MERGE_INTERVAL: u64 = 40;

/// How close a player has to be to pick something up, in blocks.
pub const PICKUP_REACH: f64 = 1.0;

/// How far an orb notices a player from, in blocks.
pub const ORB_REACH: f64 = 8.0;

/// How hard an orb is pulled at its strongest.
const ORB_PULL: f64 = 0.1;

/// An item lying on the ground.
#[derive(Component, Debug, Clone)]
pub struct DroppedItem {
    pub stack: InventorySlot,
    /// Ticks before anyone may pick it up. A thing a player threw waits longer, so it does not
    /// jump straight back into their hand.
    pub pickup_delay: u16,
    /// Ticks it has lain there.
    pub age: u32,
}

impl DroppedItem {
    /// Something that has just been dropped by the world rather than by a player.
    #[must_use]
    pub const fn new(stack: InventorySlot) -> Self {
        Self {
            stack,
            pickup_delay: PICKUP_DELAY,
            age: 0,
        }
    }

    /// Whether it has lain there long enough to give up.
    #[must_use]
    pub const fn expired(&self) -> bool {
        self.age >= LIFETIME
    }

    /// Whether anyone may pick it up yet.
    #[must_use]
    pub const fn can_be_taken(&self) -> bool {
        self.pickup_delay == 0
    }

    /// Whether it will join another of its kind.
    ///
    /// A stack that is already full has nothing to gain, and one that is about to vanish should not
    /// take a fresher one down with it.
    #[must_use]
    pub fn will_merge(&self) -> bool {
        self.age < LIFETIME && self.stack.count.0 < i32::from(MAX_STACK)
    }
}

/// The largest a stack goes before it stops taking more.
///
/// Sixty-four for almost everything; what a particular item allows is one of its components, and
/// nothing reads those yet.
pub const MAX_STACK: u8 = 64;

/// Experience lying on the ground.
#[derive(Component, Debug, Clone, Copy)]
pub struct ExperienceOrb {
    pub value: u32,
    pub age: u32,
}

impl ExperienceOrb {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self { value, age: 0 }
    }

    #[must_use]
    pub const fn expired(&self) -> bool {
        self.age >= LIFETIME
    }
}

/// The sizes experience comes in.
///
/// An amount is not one orb of that size but a handful of these, largest first, which is why
/// killing something scatters several. The numbers are the game's own and follow no formula.
const ORB_SIZES: [u32; 10] = [2477, 1237, 617, 307, 149, 73, 37, 17, 7, 3];

/// The largest orb this much experience will make.
#[must_use]
pub fn largest_orb(amount: u32) -> u32 {
    ORB_SIZES
        .into_iter()
        .find(|size| amount >= *size)
        .unwrap_or(1)
}

/// What an amount of experience breaks into, largest first.
pub fn orbs_for(amount: u32) -> impl Iterator<Item = u32> {
    let mut left = amount;
    std::iter::from_fn(move || {
        if left == 0 {
            return None;
        }
        let orb = largest_orb(left);
        left -= orb;
        Some(orb)
    })
}

/// How much an orb's speed changes this tick, being pulled towards a player.
///
/// The pull falls away with distance and stops altogether at the edge of what an orb notices, so
/// one just in range drifts and one underfoot darts.
#[must_use]
pub fn pull_towards(orb: DVec3, player_eyes: DVec3) -> Vec3A {
    let towards = player_eyes - orb;
    let distance = towards.length();
    if distance == 0.0 || distance > ORB_REACH {
        return Vec3A::ZERO;
    }
    let strength = 1.0 - distance / ORB_REACH;
    (towards.normalize() * (strength * strength * ORB_PULL)).as_vec3a()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn experience_comes_in_the_sizes_the_game_gives_it() {
        assert_eq!(largest_orb(1), 1);
        assert_eq!(largest_orb(2), 1);
        assert_eq!(largest_orb(3), 3);
        assert_eq!(largest_orb(6), 3);
        assert_eq!(largest_orb(7), 7);
        assert_eq!(largest_orb(10_000), 2477);
    }

    #[test]
    fn an_amount_breaks_into_orbs_that_add_up_to_it() {
        for amount in [1, 2, 5, 12, 100, 1234, 10_000] {
            let orbs: Vec<u32> = orbs_for(amount).collect();
            assert_eq!(
                orbs.iter().sum::<u32>(),
                amount,
                "{amount} broke into {orbs:?}"
            );
            assert!(!orbs.is_empty());
        }
    }

    #[test]
    fn a_small_amount_is_one_orb() {
        assert_eq!(orbs_for(1).collect::<Vec<_>>(), vec![1]);
        assert_eq!(orbs_for(3).collect::<Vec<_>>(), vec![3]);
    }

    #[test]
    fn a_large_amount_is_several_largest_first() {
        let orbs: Vec<u32> = orbs_for(100).collect();
        assert_eq!(orbs, vec![73, 17, 7, 3]);
    }

    #[test]
    fn nothing_at_all_makes_no_orbs() {
        assert_eq!(orbs_for(0).count(), 0);
    }

    #[test]
    fn an_orb_is_pulled_towards_a_player_and_harder_the_closer_it_is() {
        let orb = DVec3::ZERO;
        let far = pull_towards(orb, DVec3::new(7.0, 0.0, 0.0));
        let near = pull_towards(orb, DVec3::new(1.0, 0.0, 0.0));

        assert!(far.x > 0.0, "it should be pulled towards, not away");
        assert!(near.x > far.x, "and harder the closer it is");
    }

    #[test]
    fn an_orb_out_of_reach_is_not_pulled_at_all() {
        assert_eq!(
            pull_towards(DVec3::ZERO, DVec3::new(ORB_REACH + 0.1, 0.0, 0.0)),
            Vec3A::ZERO
        );
    }

    #[test]
    fn a_full_stack_has_nothing_to_gain_from_joining_another() {
        let mut full = DroppedItem::new(InventorySlot::empty());
        full.stack.count = ferrumc_net_codec::net_types::var_int::VarInt(i32::from(MAX_STACK));
        assert!(!full.will_merge());

        let half = DroppedItem::new(InventorySlot::empty());
        assert!(half.will_merge());
    }

    #[test]
    fn something_that_has_lain_there_long_enough_gives_up() {
        let mut dropped = DroppedItem::new(InventorySlot::empty());
        assert!(!dropped.expired());
        dropped.age = LIFETIME;
        assert!(dropped.expired());
        assert!(
            !dropped.will_merge(),
            "and should not take a fresher one down with it"
        );
    }
}
