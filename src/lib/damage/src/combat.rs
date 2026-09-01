//! What a swing is worth.
//!
//! A hit is not the weapon's damage. It is the weapon's damage times how far the attack has
//! recharged since the last one, plus half again if it was a critical, and it may spill onto
//! everything standing nearby if it was a sweep. Which of those apply is decided by a handful of
//! conditions that all have to hold at once, and they are the reason two swings that look identical
//! land differently.
//!
//! Nothing here touches the world. What a weapon is worth is passed in; where it comes from is
//! `Reach`, which reads the item's own attribute modifiers.

use bevy_ecs::prelude::Component;
use ferrumc_data::attributes::Attribute;
use ferrumc_data::generated::items::{AttributeModifierSlot, Item, Operation};

/// How long a fist takes to recharge, expressed as swings a second.
pub const FIST_ATTACK_SPEED: f64 = 4.0;

/// What a bare fist is worth.
pub const FIST_ATTACK_DAMAGE: f64 = 1.0;

/// How far recharged an attack has to be to count as a full one.
///
/// Not one: vanilla lets the last twentieth go, so a swing timed by eye still counts.
const FULLY_CHARGED: f32 = 0.9;

/// What a critical hit multiplies the blow by.
const CRITICAL: f32 = 1.5;

/// What a sprint hit adds to the knockback.
const SPRINT_KNOCKBACK: f32 = 0.5;

/// What any landed blow pushes with, before the weapon's own knockback.
pub const DEFAULT_KNOCKBACK: f32 = 0.4;

/// What a sweep pushes the bystanders with.
pub const SWEEP_KNOCKBACK: f32 = 0.4;

/// How far a sweep reaches past the thing that was hit, in blocks.
pub const SWEEP_REACH: f64 = 1.0;

/// How far a sweep reaches up and down past it, in blocks.
pub const SWEEP_REACH_VERTICAL: f64 = 0.25;

/// How far from the attacker a bystander may be and still be caught, in blocks squared.
pub const SWEEP_RANGE_SQUARED: f64 = 9.0;

/// How much sprinting has to be beaten for a sweep to be on, as a multiple of walking speed.
const SWEEP_SPEED_LIMIT: f64 = 2.5;

/// How much hunger a swing costs.
pub const SWING_EXHAUSTION: f32 = 0.1;

/// How far an attack has recharged, and how long since the last one.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct Swing {
    /// Ticks since the last swing.
    pub ticker: u32,
}

impl Swing {
    /// One tick passing.
    pub const fn tick(&mut self) {
        self.ticker = self.ticker.saturating_add(1);
    }

    /// A swing taken, which starts the recharge over.
    pub const fn swung(&mut self) {
        self.ticker = 0;
    }

    /// How far recharged the attack is, from nothing to one.
    ///
    /// The half tick is vanilla's: it is asking what the charge will be by the time the swing
    /// actually lands rather than what it is now.
    #[must_use]
    pub fn charge(&self, attack_speed: f64) -> f32 {
        let delay = attack_delay(attack_speed);
        if delay <= 0.0 {
            return 1.0;
        }
        ((self.ticker as f32 + 0.5) / delay).clamp(0.0, 1.0)
    }
}

/// How many ticks a weapon takes to recharge.
#[must_use]
pub fn attack_delay(attack_speed: f64) -> f32 {
    if attack_speed <= 0.0 {
        return f32::INFINITY;
    }
    (1.0 / attack_speed * 20.0) as f32
}

/// What the blow is multiplied by for having been swung early.
///
/// A fifth at rest, rising with the square of the charge, so the last part of the recharge is worth
/// far more than the first: half charged is 40% of the blow, not half of it.
#[must_use]
pub fn charge_scale(charge: f32) -> f32 {
    0.2 + charge * charge * 0.8
}

/// What the attacker is doing when they swing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Swinging {
    pub sprinting: bool,
    /// Falling, with nothing under the feet: what a critical needs.
    pub falling: bool,
    pub on_ground: bool,
    pub in_water: bool,
    pub on_a_ladder: bool,
    pub riding: bool,
    /// How fast the attacker is going along the ground, in blocks a tick.
    pub speed: f64,
    /// How fast walking is for them, which is what a sweep compares that against.
    pub walking_speed: f64,
    /// Whether what is in the main hand is a sword.
    pub holding_a_sword: bool,
}

/// What the target is.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Target {
    /// Whether it is something that lives, which is what a critical and a sweep need.
    pub living: bool,
}

/// What a swing came to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blow {
    /// What lands on the thing that was hit, before its armour.
    pub damage: f32,
    /// How hard to push it.
    pub knockback: f32,
    pub critical: bool,
    /// Whether everything standing near the target is caught too, and for how much each.
    pub sweep: Option<f32>,
}

impl Blow {
    /// Nothing at all, for a swing that could not land.
    pub const NOTHING: Self = Self {
        damage: 0.0,
        knockback: 0.0,
        critical: false,
        sweep: None,
    };
}

/// What the attacker is holding, as far as a fight is concerned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weapon {
    pub attack_damage: f64,
    pub attack_speed: f64,
    /// What the weapon adds to the push, before the half vanilla takes off it.
    pub attack_knockback: f64,
    /// What share of the blow a sweep passes on, which only a sword has.
    pub sweeping_ratio: f64,
}

impl Default for Weapon {
    /// A bare fist.
    fn default() -> Self {
        Self {
            attack_damage: FIST_ATTACK_DAMAGE,
            attack_speed: FIST_ATTACK_SPEED,
            attack_knockback: 0.0,
            sweeping_ratio: 0.0,
        }
    }
}

impl Weapon {
    /// What holding a particular item comes to.
    ///
    /// An item carries its own modifiers to the attacker's attributes, which is where a sword's
    /// six points of damage and its slower recharge live. Only what is worn in the main hand
    /// counts, and only the plain add — the two scaling operations exist but no vanilla weapon
    /// uses them, and applying one out of order would be worse than not applying it.
    #[must_use]
    pub fn in_hand(item: Option<&'static Item>) -> Self {
        let mut weapon = Self::default();
        let Some(item) = item else {
            return weapon;
        };
        for modifier in item.attribute_modifiers() {
            if !matches!(
                modifier.slot,
                AttributeModifierSlot::Any | AttributeModifierSlot::String("mainhand")
            ) || !matches!(modifier.operation, Operation::AddValue)
            {
                continue;
            }
            let onto = match modifier.r#type.name {
                "attack_damage" => &mut weapon.attack_damage,
                "attack_speed" => &mut weapon.attack_speed,
                "attack_knockback" => &mut weapon.attack_knockback,
                "sweeping_damage_ratio" => &mut weapon.sweeping_ratio,
                _ => continue,
            };
            *onto = Attribute::from_name(modifier.r#type.name)
                .map_or(*onto + modifier.amount, |attribute| {
                    attribute.clamp(*onto + modifier.amount)
                });
        }
        weapon
    }
}

/// What a swing comes to.
///
/// The order is vanilla's and it matters: the charge scales the base damage before the critical
/// multiplies it, so a half-charged critical is not half a full one. What the difficulty does to
/// the result is settled where every other blow is, in [`crate::resolve`].
#[must_use]
pub fn swing(weapon: Weapon, charge: f32, attacker: Swinging, target: Target) -> Blow {
    let full_strength = charge > FULLY_CHARGED;
    let knockback_attack = attacker.sprinting && full_strength;

    let mut damage = weapon.attack_damage as f32 * charge_scale(charge);

    // A critical needs the attacker to be on the way down under their own weight, with nothing
    // helping and nothing in the way, and it does not survive sprinting.
    let critical = full_strength
        && target.living
        && attacker.falling
        && !attacker.on_ground
        && !attacker.in_water
        && !attacker.on_a_ladder
        && !attacker.riding
        && !attacker.sprinting;
    if critical {
        damage *= CRITICAL;
    }

    // A sweep is what a sword does when it is swung at a walk: full strength, not a critical, not a
    // sprint hit, both feet down, and moving no faster than walking.
    let sweeping = full_strength
        && !critical
        && !knockback_attack
        && attacker.on_ground
        && attacker.holding_a_sword
        && attacker.speed < (attacker.walking_speed * SWEEP_SPEED_LIMIT).powi(2);
    // What a bystander takes is a share of the blow rather than the blow, and it is scaled by the
    // charge a second time.
    let sweep = sweeping.then_some((1.0 + weapon.sweeping_ratio as f32 * damage) * charge);

    let knockback = (weapon.attack_knockback as f32 / 2.0)
        + if knockback_attack {
            SPRINT_KNOCKBACK
        } else {
            0.0
        };

    Blow {
        damage,
        knockback: DEFAULT_KNOCKBACK + knockback,
        critical,
        sweep,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_sword() -> Weapon {
        Weapon {
            attack_damage: 7.0,
            attack_speed: 1.6,
            attack_knockback: 0.0,
            sweeping_ratio: 0.0,
        }
    }

    fn standing_still() -> Swinging {
        Swinging {
            on_ground: true,
            walking_speed: 0.1,
            ..Swinging::default()
        }
    }

    fn a_mob() -> Target {
        Target { living: true }
    }

    #[test]
    fn a_fist_recharges_in_five_ticks_and_a_diamond_sword_in_twelve_and_a_half() {
        assert_eq!(attack_delay(FIST_ATTACK_SPEED), 5.0);
        assert_eq!(attack_delay(1.6), 12.5);
    }

    #[test]
    fn a_swing_that_has_not_recharged_is_worth_a_fifth() {
        // Straight after a swing the charge is almost nothing, and the blow is a fifth of itself.
        let just_swung = Swing { ticker: 0 };
        let charge = just_swung.charge(1.6);
        assert!(charge < 0.1, "{charge}");
        assert!((charge_scale(charge) - 0.2).abs() < 0.02);
    }

    #[test]
    fn the_last_part_of_the_recharge_is_worth_far_more_than_the_first() {
        // Half charged is 40% of the blow, not half of it. This is why the cooldown bar matters.
        assert!((charge_scale(0.5) - 0.4).abs() < 1e-5);
        assert!((charge_scale(1.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn a_fully_recharged_swing_is_the_whole_weapon() {
        let blow = swing(a_sword(), 1.0, standing_still(), a_mob());
        assert!((blow.damage - 7.0).abs() < 1e-5);
        assert!(!blow.critical);
    }

    #[test]
    fn a_critical_needs_to_be_falling_and_not_sprinting() {
        let falling = Swinging {
            falling: true,
            on_ground: false,
            ..standing_still()
        };
        let blow = swing(a_sword(), 1.0, falling, a_mob());
        assert!(blow.critical);
        assert!((blow.damage - 10.5).abs() < 1e-5, "half again");

        let sprinting = Swinging {
            sprinting: true,
            ..falling
        };
        assert!(
            !swing(a_sword(), 1.0, sprinting, a_mob()).critical,
            "sprinting takes the critical away"
        );
    }

    #[test]
    fn a_critical_needs_a_full_charge() {
        let falling = Swinging {
            falling: true,
            on_ground: false,
            ..standing_still()
        };
        assert!(!swing(a_sword(), 0.5, falling, a_mob()).critical);
    }

    #[test]
    fn nothing_that_does_not_live_is_hit_critically() {
        let falling = Swinging {
            falling: true,
            on_ground: false,
            ..standing_still()
        };
        let boat = Target { living: false };
        assert!(!swing(a_sword(), 1.0, falling, boat).critical);
    }

    #[test]
    fn sprinting_at_full_charge_pushes_harder() {
        let plain = swing(a_sword(), 1.0, standing_still(), a_mob());
        let sprinting = Swinging {
            sprinting: true,
            ..standing_still()
        };
        let charging = swing(a_sword(), 1.0, sprinting, a_mob());
        assert!((charging.knockback - plain.knockback - SPRINT_KNOCKBACK).abs() < 1e-5);
    }

    #[test]
    fn sprinting_without_a_full_charge_pushes_no_harder() {
        let sprinting = Swinging {
            sprinting: true,
            ..standing_still()
        };
        let blow = swing(a_sword(), 0.5, sprinting, a_mob());
        assert!((blow.knockback - DEFAULT_KNOCKBACK).abs() < 1e-5);
    }

    #[test]
    fn a_sword_swung_at_a_walk_sweeps() {
        let walking = Swinging {
            holding_a_sword: true,
            speed: 0.001,
            ..standing_still()
        };
        assert!(swing(a_sword(), 1.0, walking, a_mob()).sweep.is_some());
    }

    #[test]
    fn nothing_else_sweeps() {
        let walking = Swinging {
            holding_a_sword: true,
            speed: 0.001,
            ..standing_still()
        };

        // Not while sprinting, since that is a knockback hit instead.
        let sprinting = Swinging {
            sprinting: true,
            ..walking
        };
        assert!(swing(a_sword(), 1.0, sprinting, a_mob()).sweep.is_none());

        // Not in the air, since that is a critical instead.
        let falling = Swinging {
            falling: true,
            on_ground: false,
            ..walking
        };
        assert!(swing(a_sword(), 1.0, falling, a_mob()).sweep.is_none());

        // Not with an axe.
        let axe = Swinging {
            holding_a_sword: false,
            ..walking
        };
        assert!(swing(a_sword(), 1.0, axe, a_mob()).sweep.is_none());

        // Not half charged.
        assert!(swing(a_sword(), 0.5, walking, a_mob()).sweep.is_none());
    }

    #[test]
    fn a_sword_knows_what_it_is_worth() {
        let sword = Weapon::in_hand(Item::from_registry_key("minecraft:diamond_sword"));
        assert!(
            (sword.attack_damage - 7.0).abs() < 1e-5,
            "one from the fist plus six from the sword, {sword:?}"
        );
        assert!(
            (sword.attack_speed - 1.6).abs() < 1e-5,
            "four from the arm less 2.4 from the sword, {sword:?}"
        );
    }

    #[test]
    fn an_axe_hits_harder_and_slower_than_a_sword() {
        let sword = Weapon::in_hand(Item::from_registry_key("minecraft:diamond_sword"));
        let axe = Weapon::in_hand(Item::from_registry_key("minecraft:diamond_axe"));
        assert!(axe.attack_damage > sword.attack_damage);
        assert!(axe.attack_speed < sword.attack_speed);
    }

    #[test]
    fn an_empty_hand_is_a_fist() {
        let fist = Weapon::in_hand(None);
        assert_eq!(fist.attack_damage, FIST_ATTACK_DAMAGE);
        assert_eq!(fist.attack_speed, FIST_ATTACK_SPEED);

        // And so is holding something that is not a weapon.
        let dirt = Weapon::in_hand(Item::from_registry_key("minecraft:dirt"));
        assert_eq!(dirt, fist);
    }
}
