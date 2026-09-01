//! The counters that turn standing somewhere into being hurt by it.
//!
//! Falling, drowning and burning are not events. Each is a number that creeps up or down every tick
//! and produces a blow when it crosses a line: how far something has fallen since it last touched
//! ground, how long it has held its breath, how long it has left to burn. They are kept together
//! because they are read together, once a tick, by the one system that turns them into damage.

use bevy_ecs::prelude::Component;

/// How full a pair of lungs is, in ticks of air.
pub const FULL_LUNGS: i16 = 300;

/// How far past empty the lungs go before the first mouthful of water.
///
/// Vanilla keeps counting past zero and only starts drowning a second later, which is why the last
/// bubble is not the moment the damage begins.
pub const DROWNING_AT: i16 = -20;

/// How much air a breath at the surface gives back.
pub const BREATH: i16 = 4;

/// What one mouthful of water costs.
pub const DROWNING_DAMAGE: f32 = 2.0;

/// What one tick of burning costs, and how often it is paid.
pub const BURNING_DAMAGE: f32 = 1.0;

/// How often something on fire is hurt by it, in ticks.
pub const BURN_INTERVAL: i16 = 20;

/// How long standing in fire keeps something alight, in ticks.
pub const FIRE_BURNS_FOR: i16 = 160;

/// How long standing in lava keeps something alight, in ticks.
pub const LAVA_BURNS_FOR: i16 = 300;

/// What standing in fire costs each tick.
pub const FIRE_DAMAGE: f32 = 1.0;

/// What standing in lava costs each tick.
pub const LAVA_DAMAGE: f32 = 4.0;

/// What the void costs each tick, once something is far enough below the world.
pub const VOID_DAMAGE: f32 = 4.0;

/// How far below the bottom of the world the void starts.
pub const VOID_BELOW: i32 = 64;

/// How far something falls before landing costs anything.
///
/// An attribute in vanilla, so a mob or a potion can move it; nothing moves it yet.
pub const SAFE_FALL: f64 = 3.0;

/// The counters that decide whether standing somewhere hurts.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Vitals {
    /// How far it has fallen since it last touched ground.
    pub fallen: f64,
    /// Ticks of air left, which goes negative before it starts costing anything.
    pub air: i16,
    /// Ticks it has left to burn.
    pub burning: i16,
    /// Where it was last tick, which is the only way to know how far it dropped this one.
    pub last_y: f64,
}

impl Default for Vitals {
    fn default() -> Self {
        Self {
            fallen: 0.0,
            air: FULL_LUNGS,
            burning: 0,
            last_y: 0.0,
        }
    }
}

impl Vitals {
    /// Adds to how far something has fallen, given how much it dropped this tick.
    ///
    /// Only downward movement counts, and only out of water: a dive does not hurt.
    pub fn fell(&mut self, dropped: f64, in_water: bool) {
        if !in_water && dropped < 0.0 {
            self.fallen -= dropped;
        }
    }

    /// What landing costs, and clears what was fallen.
    ///
    /// Vanilla adds a millionth of a block before flooring, so a fall of exactly the safe distance
    /// plus one block costs one rather than nothing. How far is safe and how hard the landing is
    /// are both attributes, so feather falling and a slow-falling potion move them rather than
    /// touching this.
    pub fn land(&mut self, safe: f64, multiplier: f64) -> f32 {
        let hurt = ((self.fallen + 1e-6 - safe).max(0.0) * multiplier)
            .floor()
            .max(0.0);
        self.fallen = 0.0;
        hurt as f32
    }

    /// A breath taken, or one held. Returns what a held breath costs this tick.
    ///
    /// `held_longer` is the chance a tick of holding costs nothing, which is what respiration is:
    /// an oxygen bonus of one skips half the ticks, of two skips two thirds.
    pub fn breathe(&mut self, underwater: bool, held_longer: bool) -> f32 {
        if !underwater {
            self.air = (self.air + BREATH).min(FULL_LUNGS);
            return 0.0;
        }
        if !held_longer {
            self.air -= 1;
        }
        if self.air <= DROWNING_AT {
            self.air = 0;
            return DROWNING_DAMAGE;
        }
        0.0
    }

    /// Sets something alight for at least this long. Something already burning longer keeps its own
    /// time.
    pub fn ignite(&mut self, ticks: i16) {
        self.burning = self.burning.max(ticks);
    }

    /// A tick of burning. Returns what it costs, which is nothing on nineteen ticks out of twenty.
    ///
    /// Something standing in lava is already being hurt by the lava, so it is not billed twice.
    pub fn burn(&mut self, in_lava: bool) -> f32 {
        if self.burning <= 0 {
            return 0.0;
        }
        let cost = if self.burning % BURN_INTERVAL == 0 && !in_lava {
            BURNING_DAMAGE
        } else {
            0.0
        };
        self.burning -= 1;
        cost
    }

    /// Whether it is alight, which is what a client is told.
    #[must_use]
    pub const fn on_fire(&self) -> bool {
        self.burning > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bigger_safe_distance_takes_the_sting_out_of_a_fall() {
        // What feather falling and a slow-falling potion do: move the attribute, not the code.
        let mut vitals = Vitals {
            fallen: 10.0,
            ..Vitals::default()
        };
        assert_eq!(vitals.land(SAFE_FALL, 1.0), 7.0);

        vitals.fallen = 10.0;
        assert_eq!(vitals.land(SAFE_FALL + 4.0, 1.0), 3.0);

        vitals.fallen = 10.0;
        assert_eq!(
            vitals.land(SAFE_FALL, 0.5),
            3.0,
            "and half as hard a landing"
        );

        vitals.fallen = 10.0;
        assert_eq!(vitals.land(SAFE_FALL, 0.0), 0.0, "or none at all");
    }

    #[test]
    fn holding_a_breath_longer_costs_no_air_that_tick() {
        // Respiration: the ticks it skips are the whole of the effect.
        let mut vitals = Vitals::default();
        for _ in 0..10 {
            assert_eq!(vitals.breathe(true, true), 0.0);
        }
        assert_eq!(vitals.air, FULL_LUNGS, "not one tick of air went");
    }

    #[test]
    fn a_short_fall_costs_nothing_and_a_long_one_costs_a_heart_a_block() {
        let mut vitals = Vitals {
            fallen: 3.0,
            ..Vitals::default()
        };
        assert_eq!(
            vitals.land(SAFE_FALL, 1.0),
            0.0,
            "three blocks is the free one"
        );

        vitals.fallen = 4.0;
        assert_eq!(vitals.land(SAFE_FALL, 1.0), 1.0);

        vitals.fallen = 10.0;
        assert_eq!(vitals.land(SAFE_FALL, 1.0), 7.0);
    }

    #[test]
    fn landing_clears_what_was_fallen() {
        let mut vitals = Vitals {
            fallen: 20.0,
            ..Vitals::default()
        };
        let _ = vitals.land(SAFE_FALL, 1.0);
        assert_eq!(vitals.fallen, 0.0);
        assert_eq!(
            vitals.land(SAFE_FALL, 1.0),
            0.0,
            "and the next step is not a fall"
        );
    }

    #[test]
    fn only_going_down_counts_and_only_out_of_water() {
        let mut vitals = Vitals::default();
        vitals.fell(0.5, false);
        assert_eq!(vitals.fallen, 0.0, "going up is not falling");

        vitals.fell(-2.0, true);
        assert_eq!(vitals.fallen, 0.0, "a dive is not a fall");

        vitals.fell(-2.0, false);
        assert_eq!(vitals.fallen, 2.0);
    }

    #[test]
    fn the_last_bubble_is_not_where_the_drowning_starts() {
        let mut vitals = Vitals::default();
        // Down to nothing costs nothing at all.
        for _ in 0..FULL_LUNGS {
            assert_eq!(vitals.breathe(true, false), 0.0);
        }
        assert_eq!(vitals.air, 0);

        // And then a further second of holding on before the first mouthful.
        for _ in 0..-DROWNING_AT - 1 {
            assert_eq!(vitals.breathe(true, false), 0.0);
        }
        assert_eq!(vitals.breathe(true, false), DROWNING_DAMAGE);
    }

    #[test]
    fn surfacing_fills_the_lungs_four_times_as_fast_as_holding_empties_them() {
        let mut vitals = Vitals::default();
        for _ in 0..40 {
            let _ = vitals.breathe(true, false);
        }
        assert_eq!(vitals.air, FULL_LUNGS - 40);

        for _ in 0..10 {
            assert_eq!(vitals.breathe(false, false), 0.0);
        }
        assert_eq!(vitals.air, FULL_LUNGS);
    }

    #[test]
    fn burning_costs_a_heart_a_second_not_a_heart_a_tick() {
        let mut vitals = Vitals::default();
        vitals.ignite(FIRE_BURNS_FOR);

        let mut paid = 0.0;
        let mut ticks = 0;
        while vitals.on_fire() {
            paid += vitals.burn(false);
            ticks += 1;
        }
        assert_eq!(ticks, FIRE_BURNS_FOR);
        assert_eq!(paid, 8.0, "eight seconds of fire is eight hearts of damage");
    }

    #[test]
    fn something_already_burning_longer_keeps_its_own_time() {
        let mut vitals = Vitals::default();
        vitals.ignite(LAVA_BURNS_FOR);
        vitals.ignite(FIRE_BURNS_FOR);
        assert_eq!(vitals.burning, LAVA_BURNS_FOR);
    }

    #[test]
    fn something_standing_in_lava_is_not_billed_for_the_flames_as_well() {
        let mut vitals = Vitals::default();
        vitals.ignite(LAVA_BURNS_FOR);
        let mut paid = 0.0;
        for _ in 0..LAVA_BURNS_FOR {
            paid += vitals.burn(true);
        }
        assert_eq!(paid, 0.0);
    }
}
