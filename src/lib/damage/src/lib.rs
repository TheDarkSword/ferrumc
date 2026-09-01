//! How much of a blow actually lands.
//!
//! A number of damage does not become a number of health lost. It is softened by armour, then by
//! resistance, then by whatever absorption the victim is carrying, and before any of that it may
//! not land at all — a thing that has just been hit is briefly hard to hit again, and how briefly
//! depends on what is hitting it.
//!
//! The armour step is the one worth reading twice. It is not "each point of armour takes four per
//! cent off": heavy blows cut through armour, which is why a diamond-clad player still dies to an
//! anvil. Nothing here touches the world; what softens a blow is passed in.

pub mod combat;
pub mod vitals;

use bevy_ecs::prelude::{Component, Resource};
pub use combat::{Blow, Swing, Weapon};
use ferrumc_data::generated::damage_types::{DamageType, Scaling};
pub use vitals::Vitals;

/// The most armour that counts, however much is worn.
const MOST_ARMOUR_THAT_COUNTS: f32 = 20.0;

/// What armour is divided by to become a fraction of the blow.
const ARMOUR_DIVIDER: f32 = 25.0;

/// The armour a blow cannot cut through, as a fraction of what is worn.
const ARMOUR_FLOOR: f32 = 0.2;

/// How much toughness everything has before any is worn.
const BASE_TOUGHNESS: f32 = 2.0;

/// How long a thing is hard to hit again after being hit, in ticks.
pub const INVULNERABLE_TICKS: u8 = 20;

/// Past this many ticks left, a blow has to beat the last one to land at all.
const STILL_REELING: u8 = 10;

/// What is hitting, and how hard.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub kind: DamageType,
    pub amount: f32,
}

/// What the victim has to soften it.
///
/// Nothing fills most of this in yet — armour needs attributes and protection needs enchantments —
/// so the arithmetic is exercised by its tests before it is exercised by a player.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct Defence {
    /// Armour points worn.
    pub armour: f32,
    /// Armour toughness, which is what keeps a heavy blow from cutting straight through.
    pub toughness: f32,
    /// The level of resistance, where the victim has it. One means resistance I.
    pub resistance: u8,
    /// Extra health that is spent before real health is.
    pub absorption: f32,
}

/// How long the victim is still hard to hit, and how hard the last blow was.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct Reeling {
    pub ticks: u8,
    /// What the last blow came to. A new blow has to beat this to land while still reeling.
    pub last: f32,
}

impl Reeling {
    /// One tick passing.
    pub const fn tick(&mut self) {
        self.ticks = self.ticks.saturating_sub(1);
    }
}

/// What a blow came to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Landed {
    /// Health actually lost.
    pub health: f32,
    /// Absorption spent instead of health.
    pub absorbed: f32,
}

impl Landed {
    /// Nothing at all.
    pub const NOTHING: Self = Self {
        health: 0.0,
        absorbed: 0.0,
    };

    /// Whether anything came of it.
    #[must_use]
    pub fn landed(&self) -> bool {
        self.health > 0.0 || self.absorbed > 0.0
    }
}

/// What is left of a blow after armour.
///
/// The number most people remember is four per cent off per point, and that is only true of a
/// light blow. A heavy one cuts through: the armour that counts falls by the damage divided by
/// toughness, down to a fifth of what is worn. Twenty armour stops eighty per cent of a small hit
/// and far less of a large one.
#[must_use]
pub fn after_armour(damage: f32, armour: f32, toughness: f32) -> f32 {
    if armour <= 0.0 {
        return damage;
    }
    let toughness = BASE_TOUGHNESS + toughness / 4.0;
    let counts =
        (armour - damage / toughness).clamp(armour * ARMOUR_FLOOR, MOST_ARMOUR_THAT_COUNTS);
    damage * (1.0 - counts / ARMOUR_DIVIDER)
}

/// What is left of a blow after resistance.
///
/// Each level takes a fifth off, and five levels take all of it.
#[must_use]
pub fn after_resistance(damage: f32, level: u8) -> f32 {
    if level == 0 {
        return damage;
    }
    let taken = i32::from(level) * 5;
    let left = (25 - taken).max(0);
    (damage * left as f32 / 25.0).max(0.0)
}

/// Whether a blow can touch the victim at all, before any arithmetic.
///
/// Three things stop it outright: the victim being flagged invulnerable, which almost everything
/// respects; being immune to fire, which most of the nether is; and being immune to falling, which
/// nothing that walks is. Anything that gets past here still has to get past the reeling and the
/// armour.
#[must_use]
pub fn can_be_hurt(kind: DamageType, victim: Immunities) -> bool {
    if victim.invulnerable && !kind.goes_through_invulnerability() {
        return false;
    }
    if victim.fire && kind.is_fire() {
        return false;
    }
    if victim.falling && kind.is_fall() {
        return false;
    }
    true
}

/// What a victim cannot be hurt by at all.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Immunities {
    /// Flagged invulnerable, as a creative player or a command-spawned entity is.
    pub invulnerable: bool,
    /// Unburnable, as everything native to the nether is.
    pub fire: bool,
    /// Unhurt by landing, as an iron golem is.
    pub falling: bool,
}

/// What a blow comes to, and what it leaves the victim reeling from.
///
/// The whole pipeline in one place, in vanilla's order: does it land at all, then armour, then
/// resistance, then absorption. `reeling` and `defence` are updated to what they become.
#[must_use]
pub fn resolve(hit: Hit, defence: &mut Defence, reeling: &mut Reeling) -> Landed {
    if hit.amount <= 0.0 {
        return Landed::NOTHING;
    }

    // A thing that has just been hit is briefly hard to hit again — but only briefly, and only by
    // something no worse than what hit it. A harder blow lands, less what the first one already
    // took, which is why two hits in the same moment are not two hits' worth.
    let mut amount = hit.amount;
    if !hit.kind.goes_through_the_cooldown() {
        if reeling.ticks > STILL_REELING {
            if amount <= reeling.last {
                return Landed::NOTHING;
            }
            amount -= reeling.last;
            reeling.last = hit.amount;
        } else {
            reeling.last = hit.amount;
            reeling.ticks = INVULNERABLE_TICKS;
        }
    }

    if !hit.kind.goes_through_armour() {
        amount = after_armour(amount, defence.armour, defence.toughness);
    }
    if !hit.kind.goes_through_effects() && !hit.kind.goes_through_resistance() {
        amount = after_resistance(amount, defence.resistance);
    }

    // Absorption is spent before health is, and is spent whether or not it covers the whole blow.
    let absorbed = amount.min(defence.absorption);
    defence.absorption -= absorbed;
    Landed {
        health: amount - absorbed,
        absorbed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_sword(amount: f32) -> Hit {
        Hit {
            kind: DamageType::PlayerAttack,
            amount,
        }
    }

    #[test]
    fn nothing_softens_a_blow_against_nothing() {
        let mut defence = Defence::default();
        let mut reeling = Reeling::default();
        let landed = resolve(a_sword(7.0), &mut defence, &mut reeling);
        assert_eq!(landed.health, 7.0);
    }

    #[test]
    fn armour_stops_roughly_four_per_cent_a_point_of_a_light_blow() {
        // The number everyone remembers is four per cent a point, and even a one-damage blow does
        // not quite get it: twenty armour with no toughness lets 0.22 of it through, not 0.20,
        // because the blow has already cut half a point off the armour that counts.
        let left = after_armour(1.0, 20.0, 0.0);
        assert!((left - 0.22).abs() < 1e-5, "{left}");
    }

    #[test]
    fn a_heavy_blow_cuts_through_armour() {
        // The number nobody remembers. The same twenty armour stops far less of a big hit, which
        // is why a fully armoured player still dies to an anvil.
        let light = after_armour(1.0, 20.0, 0.0) / 1.0;
        let heavy = after_armour(40.0, 20.0, 0.0) / 40.0;
        assert!(
            heavy > light,
            "a heavy blow should get through more of the armour: {heavy} vs {light}"
        );
    }

    #[test]
    fn toughness_is_what_stops_a_heavy_blow_cutting_through() {
        let plain = after_armour(20.0, 20.0, 0.0);
        let tough = after_armour(20.0, 20.0, 8.0);
        assert!(tough < plain, "toughness should hold the armour up");
    }

    #[test]
    fn armour_never_stops_more_than_four_fifths() {
        for damage in [1.0, 5.0, 20.0, 100.0] {
            let left = after_armour(damage, 20.0, 20.0);
            assert!(
                left >= damage * ARMOUR_FLOOR - 1e-5,
                "{damage} came down to {left}"
            );
        }
    }

    #[test]
    fn resistance_takes_a_fifth_a_level() {
        assert_eq!(after_resistance(10.0, 0), 10.0);
        assert!((after_resistance(10.0, 1) - 8.0).abs() < 1e-5);
        assert!((after_resistance(10.0, 4) - 2.0).abs() < 1e-5);
        assert_eq!(
            after_resistance(10.0, 5),
            0.0,
            "five levels stop everything"
        );
    }

    #[test]
    fn a_second_blow_in_the_same_moment_is_not_a_second_blows_worth() {
        let mut defence = Defence::default();
        let mut reeling = Reeling::default();

        assert_eq!(
            resolve(a_sword(6.0), &mut defence, &mut reeling).health,
            6.0
        );
        assert_eq!(
            resolve(a_sword(4.0), &mut defence, &mut reeling),
            Landed::NOTHING,
            "a weaker blow while still reeling does nothing at all"
        );
        assert_eq!(
            resolve(a_sword(10.0), &mut defence, &mut reeling).health,
            4.0,
            "a harder one lands the difference"
        );
    }

    #[test]
    fn once_the_reeling_has_passed_a_blow_lands_in_full() {
        let mut defence = Defence::default();
        let mut reeling = Reeling::default();
        let _ = resolve(a_sword(6.0), &mut defence, &mut reeling);
        for _ in 0..INVULNERABLE_TICKS {
            reeling.tick();
        }
        assert_eq!(
            resolve(a_sword(1.0), &mut defence, &mut reeling).health,
            1.0
        );
    }

    #[test]
    fn being_flagged_invulnerable_stops_almost_everything() {
        let creative = Immunities {
            invulnerable: true,
            ..Immunities::default()
        };
        assert!(!can_be_hurt(DamageType::PlayerAttack, creative));
        assert!(
            can_be_hurt(DamageType::OutOfWorld, creative),
            "the void goes through the flag; that is what it is for"
        );
    }

    #[test]
    fn what_cannot_burn_is_not_burnt_and_what_cannot_fall_is_not_hurt_by_landing() {
        let nether = Immunities {
            fire: true,
            ..Immunities::default()
        };
        assert!(!can_be_hurt(DamageType::InFire, nether));
        assert!(!can_be_hurt(DamageType::OnFire, nether));
        assert!(can_be_hurt(DamageType::Fall, nether));

        let golem = Immunities {
            falling: true,
            ..Immunities::default()
        };
        assert!(!can_be_hurt(DamageType::Fall, golem));
        assert!(can_be_hurt(DamageType::InFire, golem));
    }

    #[test]
    fn nothing_in_the_packs_skips_the_reeling() {
        // Vanilla keeps the tag but puts nothing in it, so every kind of damage waits its turn.
        // Written down because an empty tag is otherwise indistinguishable from a missing one.
        for kind in [
            DamageType::OutOfWorld,
            DamageType::GenericKill,
            DamageType::PlayerAttack,
            DamageType::Fall,
        ] {
            assert!(!kind.goes_through_the_cooldown(), "{kind:?}");
        }
    }

    #[test]
    fn absorption_is_spent_before_health() {
        let mut defence = Defence {
            absorption: 4.0,
            ..Defence::default()
        };
        let mut reeling = Reeling::default();

        let landed = resolve(a_sword(6.0), &mut defence, &mut reeling);
        assert_eq!(landed.absorbed, 4.0);
        assert_eq!(landed.health, 2.0);
        assert_eq!(defence.absorption, 0.0, "and is used up");
    }

    #[test]
    fn armour_does_not_soften_what_goes_around_it() {
        let mut plated = Defence {
            armour: 20.0,
            ..Defence::default()
        };
        let mut reeling = Reeling::default();
        let starving = Hit {
            kind: DamageType::Starve,
            amount: 1.0,
        };
        assert_eq!(
            resolve(starving, &mut plated, &mut reeling).health,
            1.0,
            "armour is no help against an empty stomach"
        );
    }
}

/// How hard the world hits.
///
/// Peaceful is not "no damage": it stops what a mob does and leaves falling, drowning and the void
/// exactly where they are.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Resource)]
pub enum Difficulty {
    Peaceful,
    Easy,
    #[default]
    Normal,
    Hard,
}

impl Difficulty {
    /// The number a client reads.
    #[must_use]
    pub const fn wire_id(self) -> i32 {
        match self {
            Self::Peaceful => 0,
            Self::Easy => 1,
            Self::Normal => 2,
            Self::Hard => 3,
        }
    }

    /// The name an operator writes in the config.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "peaceful" => Some(Self::Peaceful),
            "easy" => Some(Self::Easy),
            "normal" => Some(Self::Normal),
            "hard" => Some(Self::Hard),
            _ => None,
        }
    }
}

/// What a blow comes to at a given difficulty.
///
/// Only some kinds move with it — a mob's blow does, a player's and the world's own hazards do not
/// — and which is which comes off the kind. Easy is not half: it is half plus one, and never more
/// than the blow itself, so a one-point hit stays a one-point hit.
#[must_use]
pub fn scale_for(damage: f32, kind: DamageType, by_a_mob: bool, difficulty: Difficulty) -> f32 {
    let moves = match kind.scaling() {
        Scaling::Never => false,
        Scaling::WhenCausedByLivingNonPlayer => by_a_mob,
        Scaling::Always => true,
    };
    if !moves {
        return damage;
    }
    match difficulty {
        Difficulty::Peaceful => 0.0,
        Difficulty::Easy => (damage / 2.0 + 1.0).min(damage),
        Difficulty::Normal => damage,
        Difficulty::Hard => damage * 3.0 / 2.0,
    }
}

#[cfg(test)]
mod difficulty_tests {
    use super::*;

    #[test]
    fn only_what_a_mob_did_moves_with_the_difficulty() {
        // A zombie's fist is softened on easy.
        assert_eq!(
            scale_for(4.0, DamageType::MobAttack, true, Difficulty::Easy),
            3.0
        );
        // The same blow from a player is not.
        assert_eq!(
            scale_for(4.0, DamageType::MobAttack, false, Difficulty::Easy),
            4.0
        );
    }

    #[test]
    fn easy_is_half_plus_one_and_never_more_than_the_blow() {
        assert_eq!(
            scale_for(10.0, DamageType::MobAttack, true, Difficulty::Easy),
            6.0
        );
        assert_eq!(
            scale_for(1.0, DamageType::MobAttack, true, Difficulty::Easy),
            1.0,
            "half plus one would be more than the blow, so the blow stands"
        );
    }

    #[test]
    fn hard_is_half_again_and_peaceful_is_nothing() {
        assert_eq!(
            scale_for(4.0, DamageType::MobAttack, true, Difficulty::Hard),
            6.0
        );
        assert_eq!(
            scale_for(4.0, DamageType::MobAttack, true, Difficulty::Peaceful),
            0.0
        );
    }

    #[test]
    fn peaceful_does_not_stop_the_world_hurting_anyone() {
        // The one thing people get wrong about peaceful: it turns off mobs, not gravity.
        for kind in [DamageType::Fall, DamageType::Drown, DamageType::OutOfWorld] {
            assert_eq!(
                scale_for(6.0, kind, false, Difficulty::Peaceful),
                6.0,
                "{kind:?}"
            );
        }
    }

    #[test]
    fn a_name_an_operator_writes_is_understood() {
        assert_eq!(Difficulty::from_name("Hard"), Some(Difficulty::Hard));
        assert_eq!(Difficulty::from_name("nonsense"), None);
    }
}
