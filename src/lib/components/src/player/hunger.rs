//! How hungry a player is, and what that does to them.
//!
//! Three numbers rather than the one a player sees. **Exhaustion** is what actions cost and it is
//! invisible; every four points of it spend one point of **saturation**, which is also invisible;
//! and only when saturation is gone does the visible **food level** start dropping. That is why a
//! full player can sprint for a long while before a single shank moves.
//!
//! Healing runs off the same numbers in two speeds: a player with saturation left heals eight times
//! a second and pays for it, and a player merely well fed heals once every four seconds. A player
//! with nothing left starves, and how far starvation goes is the difficulty's to say.

use bevy_ecs::prelude::Component;
use bitcode_derive::{Decode, Encode};
use ferrumc_damage::Difficulty;

/// The most food a stomach holds.
pub const FULL: u8 = 20;

/// What a stomach starts with in saturation.
pub const START_SATURATION: f32 = 5.0;

/// How much exhaustion one point of saturation costs.
pub const EXHAUSTION_PER_POINT: f32 = 4.0;

/// The most exhaustion that is kept at once.
pub const MOST_EXHAUSTION: f32 = 40.0;

/// How full a player has to be to heal at all.
pub const HEALS_AT: u8 = 18;

/// How full a player has to be to sprint.
pub const SPRINTS_ABOVE: u8 = 6;

/// How often a merely well fed player heals, in ticks.
pub const SLOW_HEAL: u8 = 80;

/// How often a player with saturation left heals, in ticks.
pub const FAST_HEAL: u8 = 10;

/// What one slow heal costs in exhaustion.
pub const EXHAUSTION_HEAL: f32 = 6.0;

/// The most saturation one fast heal spends.
const FAST_HEAL_SPENDS: f32 = 6.0;

/// What one block of sprinting costs.
pub const EXHAUSTION_SPRINT: f32 = 0.1;

/// What one block of swimming costs.
pub const EXHAUSTION_SWIM: f32 = 0.01;

/// What a jump costs, and what a jump while sprinting costs.
pub const EXHAUSTION_JUMP: f32 = 0.05;
pub const EXHAUSTION_SPRINT_JUMP: f32 = 0.2;

/// What breaking a block costs.
pub const EXHAUSTION_MINE: f32 = 0.005;

/// What a swing costs.
pub const EXHAUSTION_ATTACK: f32 = 0.1;

#[derive(Component, Debug, Clone, Copy, Decode, Encode)]
pub struct Hunger {
    /// 0-20 (half-shanks)
    pub level: u8,
    /// 0.0-5.0 (for regeneration)
    pub saturation: f32,
    /// 0.0-4.0 (accumulates before saturation/hunger drain)
    pub exhaustion: f32,
    /// Ticks since the last heal or the last bite of starvation.
    pub since: u8,
}

impl Default for Hunger {
    fn default() -> Self {
        Self {
            level: FULL,
            saturation: START_SATURATION,
            exhaustion: 0.0,
            since: 0,
        }
    }
}

/// What a tick of being hungry comes to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fed {
    /// Nothing this tick.
    Nothing,
    /// Heals this much.
    Heal(f32),
    /// One point of starvation.
    Starve,
}

impl Hunger {
    /// Something was done that costs energy.
    pub fn spend(&mut self, amount: f32) {
        self.exhaustion = (self.exhaustion + amount).min(MOST_EXHAUSTION);
    }

    /// What sprinting or swimming a distance costs.
    ///
    /// Walking and crouching cost nothing at all, which is the part people do not expect.
    pub fn travelled(&mut self, blocks: f64, sprinting: bool, swimming: bool) {
        // Vanilla counts distance in hundredths of a block and rounds, so a step too small to
        // register costs nothing rather than a fraction.
        let hundredths = (blocks * 100.0).round();
        if hundredths <= 0.0 {
            return;
        }
        let per_block = if swimming {
            EXHAUSTION_SWIM
        } else if sprinting {
            EXHAUSTION_SPRINT
        } else {
            return;
        };
        self.spend(per_block * hundredths as f32 * 0.01);
    }

    /// Eating something.
    ///
    /// Saturation never goes above the food level, which is why eating a steak on a nearly full
    /// stomach wastes most of it.
    pub fn eat(&mut self, nutrition: u8, saturation: f32) {
        self.level = (self.level + nutrition).min(FULL);
        self.saturation = (self.saturation + saturation).clamp(0.0, f32::from(self.level));
    }

    /// Whether a player is full enough to sprint.
    #[must_use]
    pub const fn can_sprint(&self) -> bool {
        self.level > SPRINTS_ABOVE
    }

    /// One tick passing.
    ///
    /// Returns what it comes to. `hurt` is whether the player has any health to make up; `health`
    /// is what they have, because how far starvation goes depends on it and on the difficulty.
    pub fn tick(&mut self, hurt: bool, health: f32, difficulty: Difficulty) -> Fed {
        // Every four points of exhaustion spend one point of saturation, and only once saturation
        // is gone does the visible bar move.
        if self.exhaustion > EXHAUSTION_PER_POINT {
            self.exhaustion -= EXHAUSTION_PER_POINT;
            if self.saturation > 0.0 {
                self.saturation = (self.saturation - 1.0).max(0.0);
            } else if difficulty != Difficulty::Peaceful {
                self.level = self.level.saturating_sub(1);
            }
        }

        if self.saturation > 0.0 && hurt && self.level >= FULL {
            // Full and with saturation to burn: heals fast and pays for it.
            self.since += 1;
            if self.since >= FAST_HEAL {
                let spent = self.saturation.min(FAST_HEAL_SPENDS);
                self.spend(spent);
                self.since = 0;
                return Fed::Heal(spent / FAST_HEAL_SPENDS);
            }
        } else if self.level >= HEALS_AT && hurt {
            self.since += 1;
            if self.since >= SLOW_HEAL {
                self.spend(EXHAUSTION_HEAL);
                self.since = 0;
                return Fed::Heal(1.0);
            }
        } else if self.level == 0 {
            self.since += 1;
            if self.since >= SLOW_HEAL {
                self.since = 0;
                // Starvation stops short of killing except on hard: normal leaves a player on one
                // heart, easy on five.
                let kills = match difficulty {
                    Difficulty::Hard => true,
                    Difficulty::Normal => health > 1.0,
                    _ => health > 10.0,
                };
                if kills {
                    return Fed::Starve;
                }
            }
        } else {
            self.since = 0;
        }

        Fed::Nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhaustion_eats_saturation_before_it_eats_the_visible_bar() {
        let mut hunger = Hunger::default();
        hunger.spend(EXHAUSTION_PER_POINT + 0.1);
        hunger.tick(false, 20.0, Difficulty::Normal);

        assert_eq!(hunger.level, FULL, "the bar has not moved");
        assert_eq!(hunger.saturation, START_SATURATION - 1.0);
    }

    #[test]
    fn the_bar_only_moves_once_saturation_is_gone() {
        let mut hunger = Hunger {
            saturation: 0.0,
            ..Hunger::default()
        };
        hunger.spend(EXHAUSTION_PER_POINT + 0.1);
        hunger.tick(false, 20.0, Difficulty::Normal);
        assert_eq!(hunger.level, FULL - 1);
    }

    #[test]
    fn peaceful_does_not_move_the_bar_at_all() {
        let mut hunger = Hunger {
            saturation: 0.0,
            ..Hunger::default()
        };
        hunger.spend(EXHAUSTION_PER_POINT + 0.1);
        hunger.tick(false, 20.0, Difficulty::Peaceful);
        assert_eq!(hunger.level, FULL);
    }

    #[test]
    fn walking_costs_nothing_and_sprinting_costs_a_tenth_a_block() {
        let mut hunger = Hunger::default();
        hunger.travelled(10.0, false, false);
        assert_eq!(hunger.exhaustion, 0.0, "walking is free");

        hunger.travelled(10.0, true, false);
        assert!(
            (hunger.exhaustion - 1.0).abs() < 1e-4,
            "ten blocks sprinted, {}",
            hunger.exhaustion
        );
    }

    #[test]
    fn a_step_too_small_to_count_costs_nothing() {
        let mut hunger = Hunger::default();
        hunger.travelled(0.001, true, false);
        assert_eq!(hunger.exhaustion, 0.0);
    }

    #[test]
    fn eating_on_a_nearly_full_stomach_wastes_the_food_but_not_the_saturation() {
        let mut hunger = Hunger {
            level: 19,
            saturation: 0.0,
            ..Hunger::default()
        };
        // A steak: eight nutrition, twelve point eight saturation.
        hunger.eat(8, 12.8);
        assert_eq!(hunger.level, FULL, "one shank of the eight landed");
        assert_eq!(hunger.saturation, 12.8, "the saturation all did");
    }

    #[test]
    fn saturation_never_goes_above_the_food_level() {
        // Which is what stops a half-starved player banking saturation they cannot see.
        let mut hunger = Hunger {
            level: 3,
            saturation: 0.0,
            ..Hunger::default()
        };
        hunger.eat(0, 12.8);
        assert_eq!(hunger.saturation, 3.0);
    }

    #[test]
    fn a_full_player_with_saturation_heals_eight_times_a_second() {
        let mut hunger = Hunger::default();
        let healed = (0..FAST_HEAL)
            .map(|_| hunger.tick(true, 10.0, Difficulty::Normal))
            .last();
        assert!(matches!(healed, Some(Fed::Heal(_))), "{healed:?}");
    }

    #[test]
    fn a_merely_well_fed_player_heals_once_every_four_seconds() {
        let mut hunger = Hunger {
            level: HEALS_AT,
            saturation: 0.0,
            ..Hunger::default()
        };
        for _ in 0..SLOW_HEAL - 1 {
            assert_eq!(hunger.tick(true, 10.0, Difficulty::Normal), Fed::Nothing);
        }
        assert_eq!(hunger.tick(true, 10.0, Difficulty::Normal), Fed::Heal(1.0));
    }

    #[test]
    fn a_player_who_is_not_hurt_does_not_heal_and_spends_nothing() {
        let mut hunger = Hunger::default();
        for _ in 0..200 {
            assert_eq!(hunger.tick(false, 20.0, Difficulty::Normal), Fed::Nothing);
        }
        assert_eq!(hunger.exhaustion, 0.0);
    }

    #[test]
    fn an_empty_stomach_starves_down_to_the_difficultys_floor() {
        let starves_at = |health: f32, difficulty: Difficulty| {
            let mut hunger = Hunger {
                level: 0,
                saturation: 0.0,
                ..Hunger::default()
            };
            (0..SLOW_HEAL)
                .map(|_| hunger.tick(false, health, difficulty))
                .any(|fed| fed == Fed::Starve)
        };

        // Hard kills; normal stops at one heart; easy stops at five.
        assert!(starves_at(0.5, Difficulty::Hard));
        assert!(!starves_at(1.0, Difficulty::Normal));
        assert!(starves_at(1.5, Difficulty::Normal));
        assert!(!starves_at(10.0, Difficulty::Easy));
        assert!(starves_at(10.5, Difficulty::Easy));
    }

    #[test]
    fn a_hungry_player_cannot_sprint() {
        let mut hunger = Hunger::default();
        assert!(hunger.can_sprint());
        hunger.level = SPRINTS_ABOVE;
        assert!(!hunger.can_sprint());
    }
}
