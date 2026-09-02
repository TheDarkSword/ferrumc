//! What a potion does, and for how long.
//!
//! An effect is a level and a countdown. Most of what an effect *is* is a set of modifiers on the
//! holder's attributes — speed moves `movement_speed`, strength moves `attack_damage` — and only a
//! handful do anything else: regeneration heals, poison and wither hurt, hunger and saturation move
//! a stomach. Those seven are in [`Tick`]; everything else is arithmetic the attribute system
//! already does.
//!
//! The part worth reading twice is what happens when the same effect is applied twice. A stronger
//! but shorter one does not replace a weaker longer one — it **hides** it, and the weaker one comes
//! back when the stronger runs out. That is why drinking a splash of swiftness II over a long
//! swiftness I leaves the swiftness I still running afterwards.

use bevy_ecs::prelude::Component;
use bitcode_derive::{Decode, Encode};
use ferrumc_data::generated::effects::Effect;

/// What a duration of -1 means, which is forever.
pub const FOREVER: i32 = -1;

/// The strongest an effect goes.
pub const MAX_AMPLIFIER: u8 = 255;

/// What is left of one effect on one entity.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct Instance {
    /// Zero is level one, which is how the wire counts it and how the name reads: amplifier 0 is
    /// "Speed I".
    pub amplifier: u8,
    /// Ticks left, or [`FOREVER`].
    pub duration: i32,
    /// From a beacon or a conduit, which a client draws faintly.
    pub ambient: bool,
    /// Whether the swirling particles are shown.
    pub visible: bool,
    /// Whether the icon is shown in the corner.
    pub show_icon: bool,
    /// Weaker, longer applications waiting behind this one, nearest first.
    ///
    /// A stack rather than a chain of boxes: each one comes forward in turn as the one in front of
    /// it runs out, and almost always there are none.
    behind: Vec<Waiting>,
}

/// One application waiting behind a stronger one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
struct Waiting {
    amplifier: u8,
    duration: i32,
    ambient: bool,
    visible: bool,
    show_icon: bool,
}

impl Waiting {
    const fn of(instance: &Instance) -> Self {
        Self {
            amplifier: instance.amplifier,
            duration: instance.duration,
            ambient: instance.ambient,
            visible: instance.visible,
            show_icon: instance.show_icon,
        }
    }

    const fn forever(self) -> bool {
        self.duration == FOREVER
    }

    /// Whether this one has less left than an application taking over.
    const fn shorter_than(self, other: &Instance) -> bool {
        !self.forever() && (self.duration < other.duration || other.forever())
    }
}

impl Instance {
    /// A plain effect, as a potion gives it.
    #[must_use]
    pub const fn new(amplifier: u8, duration: i32) -> Self {
        Self {
            amplifier,
            duration,
            ambient: false,
            visible: true,
            show_icon: true,
            behind: Vec::new(),
        }
    }

    /// Whether it is still running.
    #[must_use]
    pub const fn running(&self) -> bool {
        self.duration == FOREVER || self.duration > 0
    }

    /// Whether it never runs out.
    #[must_use]
    pub const fn forever(&self) -> bool {
        self.duration == FOREVER
    }

    /// Whether this one has less left than another.
    ///
    /// One that never runs out has more left than anything, and less left than nothing.
    #[must_use]
    fn shorter_than(&self, other: &Self) -> bool {
        !self.forever() && (self.duration < other.duration || other.forever())
    }

    /// Takes over from another application of the same effect.
    ///
    /// Vanilla's rule in full: a stronger one wins and hides a longer weaker one behind it; an
    /// equally strong longer one just extends; and a weaker longer one waits behind whatever is in
    /// front of it. Returns whether anything a client can see changed.
    pub fn update(&mut self, taking_over: &Self) -> bool {
        let mut changed = false;

        if taking_over.amplifier > self.amplifier {
            // The stronger one goes in front. If it also lasts longer there is nothing to come
            // back to, so the weaker one is simply dropped.
            if taking_over.shorter_than(self) {
                self.behind.insert(0, Waiting::of(self));
            }
            self.amplifier = taking_over.amplifier;
            self.duration = taking_over.duration;
            changed = true;
        } else if self.shorter_than(taking_over) {
            if taking_over.amplifier == self.amplifier {
                self.duration = taking_over.duration;
                changed = true;
            } else {
                // Weaker than what is in front, so it waits — in front of anything already waiting
                // that it outlasts or outdoes, which is the same rule one level down.
                let waiting = Waiting::of(taking_over);
                let at = self
                    .behind
                    .iter()
                    .position(|held| {
                        waiting.amplifier > held.amplifier || held.shorter_than(taking_over)
                    })
                    .unwrap_or(self.behind.len());
                self.behind.insert(at, waiting);
            }
        }

        if (!taking_over.ambient && self.ambient) || changed {
            self.ambient = taking_over.ambient;
            changed = true;
        }
        if taking_over.visible != self.visible {
            self.visible = taking_over.visible;
            changed = true;
        }
        if taking_over.show_icon != self.show_icon {
            self.show_icon = taking_over.show_icon;
            changed = true;
        }
        changed
    }

    /// One tick passing. Returns whether whatever is behind it has come forward.
    fn tick_down(&mut self) -> bool {
        if self.duration != FOREVER {
            self.duration -= 1;
        }
        if self.duration == 0 && !self.behind.is_empty() {
            let next = self.behind.remove(0);
            self.amplifier = next.amplifier;
            self.duration = next.duration;
            self.ambient = next.ambient;
            self.visible = next.visible;
            self.show_icon = next.show_icon;
            return true;
        }
        false
    }
}

/// What an effect does on the ticks it does anything.
///
/// Only these seven do anything beyond moving a number. Everything else is a set of attribute
/// modifiers, which the attribute system applies without any help.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tick {
    /// Nothing beyond whatever it moves.
    Nothing,
    /// Heals this much, and only up to full.
    Heal(f32),
    /// Hurts this much, and — for poison — never to death.
    Hurt { amount: f32, spares: bool },
    /// Withers this much, which does kill.
    Wither(f32),
    /// Makes a player this much hungrier.
    Hunger(f32),
    /// Feeds a player this much.
    Feed(u8),
    /// Tops up absorption to this much, once when it starts.
    Absorb(f32),
}

/// Whether this tick is one the effect does anything on, and what.
///
/// The interval halves with each level, so regeneration at level five is doing something every
/// other tick and at higher levels every tick. `age` is the countdown for an effect that runs out
/// and the entity's own age for one that does not, which is vanilla's way of keeping an endless
/// effect on a steady beat.
#[must_use]
pub fn tick(effect: Effect, amplifier: u8, age: i32) -> Tick {
    let every = |base: i32| {
        let interval = base >> u32::from(amplifier.min(31));
        interval <= 0 || age % interval == 0
    };

    match effect {
        Effect::Regeneration if every(50) => Tick::Heal(1.0),
        Effect::Poison if every(25) => Tick::Hurt {
            amount: 1.0,
            spares: true,
        },
        Effect::Wither if every(40) => Tick::Wither(1.0),
        Effect::Hunger => Tick::Hunger(0.005 * f32::from(amplifier + 1)),
        _ => Tick::Nothing,
    }
}

/// What an effect does the moment it is applied.
///
/// Three of the forty land all at once and are never held: healing, harming and saturation. A
/// fourth, absorption, is held but does its work once — it tops up the extra health and then only
/// lasts as long as that health does.
#[must_use]
pub fn on_added(effect: Effect, amplifier: u8) -> Tick {
    // The shift is what makes a level of healing worth double the last; capped so a silly
    // amplifier from a command cannot overflow it.
    let doubling = |base: u16| f32::from(base << u32::from(amplifier.min(12)));
    match effect {
        Effect::InstantHealth => Tick::Heal(doubling(4)),
        Effect::InstantDamage => Tick::Hurt {
            amount: doubling(6),
            spares: false,
        },
        Effect::Saturation => Tick::Feed(amplifier + 1),
        Effect::Absorption => Tick::Absorb(4.0 * f32::from(amplifier + 1)),
        _ => Tick::Nothing,
    }
}

/// Every effect on one entity.
///
/// A flat list rather than a map: nothing carries more than a handful, and a walk over a few
/// entries beats hashing. Kept in the order the effects' own numbers run, which is the order a
/// client wants them.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq, Encode, Decode)]
pub struct ActiveEffects {
    held: Vec<(Effect, Instance)>,
}

/// What a tick came to: what each effect wants done, and what a client has to be told.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ticked {
    /// What each effect wants done this tick, for the few that want anything.
    pub doing: Vec<(Effect, Tick)>,
    /// What has to be told to a client: an effect gone, or one come forward from behind another.
    pub told: Vec<(Effect, Change)>,
}

/// What changed about one effect, which is what has to be sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Newly applied, or changed enough to be worth saying again.
    Applied,
    /// Run out.
    Gone,
}

impl ActiveEffects {
    /// Applies an effect, taking over from one already there.
    ///
    /// Returns what it does the moment it lands, and whether anything a client can see changed. An
    /// effect that lands all at once is never held: it does its work and is gone.
    pub fn add(&mut self, effect: Effect, instance: Instance) -> (Tick, bool) {
        let at_once = on_added(effect, instance.amplifier);
        if effect.is_instant() {
            return (at_once, false);
        }
        (at_once, self.take_on(effect, instance))
    }

    /// Puts an effect in the set, taking over from one already there.
    fn take_on(&mut self, effect: Effect, instance: Instance) -> bool {
        match self.held.iter_mut().find(|(held, _)| *held == effect) {
            Some((_, held)) => held.update(&instance),
            None => {
                let at = self
                    .held
                    .partition_point(|(held, _)| held.id() < effect.id());
                self.held.insert(at, (effect, instance));
                true
            }
        }
    }

    /// Takes an effect away outright, hidden ones and all — which is what milk does.
    pub fn remove(&mut self, effect: Effect) -> bool {
        let before = self.held.len();
        self.held.retain(|(held, _)| *held != effect);
        self.held.len() != before
    }

    /// Takes every effect away.
    pub fn clear(&mut self) -> Vec<Effect> {
        self.held.drain(..).map(|(effect, _)| effect).collect()
    }

    /// One effect, where it is held.
    #[must_use]
    pub fn get(&self, effect: Effect) -> Option<&Instance> {
        self.held
            .iter()
            .find(|(held, _)| *held == effect)
            .map(|(_, instance)| instance)
    }

    /// What level of an effect is held, where it is held at all.
    ///
    /// One means level one, so a caller does not have to remember that zero is a level.
    #[must_use]
    pub fn level(&self, effect: Effect) -> Option<u8> {
        self.get(effect)
            .map(|held| held.amplifier.saturating_add(1))
    }

    /// Whether it is held at all.
    #[must_use]
    pub fn has(&self, effect: Effect) -> bool {
        self.get(effect).is_some()
    }

    /// Everything held.
    pub fn iter(&self) -> impl Iterator<Item = (Effect, &Instance)> {
        self.held
            .iter()
            .map(|(effect, instance)| (*effect, instance))
    }

    /// Whether nothing is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// One tick passing on everything held.
    ///
    /// Returns what each effect wants done this tick, and what has to be told to a client. An
    /// effect that runs out is dropped and reported gone; one that a stronger application was
    /// hiding comes forward and is reported applied again.
    pub fn tick(&mut self, age: i32) -> Ticked {
        let mut doing = Vec::new();
        let mut told = Vec::new();

        for (effect, instance) in &mut self.held {
            if !instance.running() {
                continue;
            }
            // An endless effect keeps time by the entity's own age, so it does not simply never
            // reach the interval.
            let beat = if instance.forever() {
                age
            } else {
                instance.duration
            };
            let what = tick(*effect, instance.amplifier, beat);
            if what != Tick::Nothing {
                doing.push((*effect, what));
            }
            if instance.tick_down() {
                told.push((*effect, Change::Applied));
            }
        }

        self.held.retain(|(effect, instance)| {
            let keep = instance.running();
            if !keep {
                told.push((*effect, Change::Gone));
            }
            keep
        });

        Ticked { doing, told }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stronger_shorter_effect_hides_a_weaker_longer_one() {
        // Swiftness II from a splash potion, over a long swiftness I.
        let mut held = Instance::new(0, 200);
        assert!(held.update(&Instance::new(1, 20)));
        assert_eq!(held.amplifier, 1);
        assert_eq!(held.duration, 20);

        // And when the stronger one runs out, the weaker one is still going.
        for _ in 0..20 {
            held.tick_down();
        }
        assert_eq!(held.amplifier, 0, "the weaker one came back");
        assert_eq!(held.duration, 200);
    }

    #[test]
    fn a_stronger_longer_effect_simply_replaces() {
        let mut held = Instance::new(0, 20);
        held.update(&Instance::new(1, 200));
        assert_eq!(held.amplifier, 1);
        assert_eq!(held.duration, 200);

        for _ in 0..200 {
            held.tick_down();
        }
        assert!(!held.running(), "nothing was waiting behind it");
    }

    #[test]
    fn the_same_level_for_longer_just_extends() {
        let mut held = Instance::new(1, 20);
        assert!(held.update(&Instance::new(1, 200)));
        assert_eq!(held.duration, 200);
        assert_eq!(held.amplifier, 1);
    }

    #[test]
    fn a_weaker_shorter_effect_changes_nothing() {
        let mut held = Instance::new(1, 200);
        assert!(!held.update(&Instance::new(0, 20)));
        assert_eq!(held.amplifier, 1);
        assert_eq!(held.duration, 200);
    }

    #[test]
    fn a_weaker_longer_effect_waits_behind() {
        let mut held = Instance::new(1, 20);
        held.update(&Instance::new(0, 200));
        assert_eq!(held.amplifier, 1, "the stronger one is still in front");

        for _ in 0..20 {
            held.tick_down();
        }
        assert_eq!(held.amplifier, 0);
        assert_eq!(held.duration, 200);
    }

    #[test]
    fn something_endless_never_runs_out() {
        let mut held = Instance::new(0, FOREVER);
        for _ in 0..10_000 {
            held.tick_down();
        }
        assert!(held.running());
        assert_eq!(held.duration, FOREVER);
    }

    #[test]
    fn regeneration_heals_faster_the_stronger_it_is() {
        let heals_in = |amplifier: u8| {
            (0..200)
                .filter(|age| tick(Effect::Regeneration, amplifier, *age) != Tick::Nothing)
                .count()
        };
        assert_eq!(heals_in(0), 4, "once every fifty ticks");
        assert_eq!(heals_in(1), 8, "twice as often");
        assert_eq!(
            heals_in(2),
            17,
            "every twelfth tick, which does not divide evenly"
        );
    }

    #[test]
    fn poison_spares_and_wither_does_not() {
        assert_eq!(
            tick(Effect::Poison, 0, 0),
            Tick::Hurt {
                amount: 1.0,
                spares: true
            }
        );
        assert_eq!(tick(Effect::Wither, 0, 0), Tick::Wither(1.0));
    }

    #[test]
    fn healing_doubles_with_each_level_and_harming_starts_higher() {
        assert_eq!(on_added(Effect::InstantHealth, 0), Tick::Heal(4.0));
        assert_eq!(on_added(Effect::InstantHealth, 1), Tick::Heal(8.0));
        assert_eq!(
            on_added(Effect::InstantDamage, 0),
            Tick::Hurt {
                amount: 6.0,
                spares: false
            }
        );
    }

    #[test]
    fn something_that_lands_all_at_once_is_never_held() {
        let mut held = ActiveEffects::default();
        let (at_once, changed) = held.add(Effect::InstantHealth, Instance::new(0, 1));
        assert_eq!(at_once, Tick::Heal(4.0), "it does its work");
        assert!(!changed);
        assert!(held.is_empty(), "and is gone");
    }

    #[test]
    fn absorption_tops_up_once_rather_than_every_tick() {
        let mut held = ActiveEffects::default();
        let (at_once, _) = held.add(Effect::Absorption, Instance::new(1, 100));
        assert_eq!(at_once, Tick::Absorb(8.0), "four a level");

        let Ticked { doing, .. } = held.tick(0);
        assert!(
            doing.is_empty(),
            "and does nothing after, so what it gave can be spent"
        );
    }

    #[test]
    fn most_effects_do_nothing_beyond_what_they_move() {
        for effect in [
            Effect::Speed,
            Effect::Strength,
            Effect::Resistance,
            Effect::FireResistance,
            Effect::Invisibility,
        ] {
            assert_eq!(tick(effect, 0, 0), Tick::Nothing, "{effect:?}");
        }
    }

    #[test]
    fn a_set_keeps_its_effects_in_the_order_a_client_wants() {
        let mut held = ActiveEffects::default();
        held.add(Effect::Strength, Instance::new(0, 100));
        held.add(Effect::Speed, Instance::new(0, 100));
        held.add(Effect::Regeneration, Instance::new(0, 100));

        let order: Vec<u16> = held.iter().map(|(effect, _)| effect.id()).collect();
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(order, sorted);
    }

    #[test]
    fn an_effect_that_runs_out_says_so_once() {
        let mut held = ActiveEffects::default();
        held.add(Effect::Speed, Instance::new(0, 2));

        let Ticked { told, .. } = held.tick(0);
        assert!(told.is_empty(), "one tick left is still running");

        let Ticked { told, .. } = held.tick(1);
        assert_eq!(told, vec![(Effect::Speed, Change::Gone)]);
        assert!(held.is_empty());

        let Ticked { told, .. } = held.tick(2);
        assert!(told.is_empty(), "and is not reported twice");
    }

    #[test]
    fn one_coming_forward_from_behind_is_worth_telling_a_client() {
        let mut held = ActiveEffects::default();
        held.add(Effect::Speed, Instance::new(0, 100));
        held.add(Effect::Speed, Instance::new(1, 2));

        let Ticked { told, .. } = held.tick(0);
        assert!(told.is_empty());
        let Ticked { told, .. } = held.tick(1);
        assert_eq!(told, vec![(Effect::Speed, Change::Applied)]);
        assert_eq!(held.level(Effect::Speed), Some(1));
    }

    #[test]
    fn three_applications_come_back_strongest_first() {
        // A long weak one, a middling one, and a short strong one on top. Each comes forward as
        // the one in front of it runs out.
        let mut held = Instance::new(0, 300);
        held.update(&Instance::new(1, 100));
        held.update(&Instance::new(2, 10));

        assert_eq!(held.amplifier, 2);
        for _ in 0..10 {
            held.tick_down();
        }
        assert_eq!(held.amplifier, 1, "the middling one");
        for _ in 0..100 {
            held.tick_down();
        }
        assert_eq!(held.amplifier, 0, "and then the weak long one");
        assert_eq!(held.duration, 300);
    }

    #[test]
    fn a_set_survives_being_written_out_and_read_back() {
        // A player carries their effects across a relog, hidden ones and all.
        let mut held = ActiveEffects::default();
        held.add(Effect::Speed, Instance::new(0, 300));
        held.add(Effect::Speed, Instance::new(2, 10));
        held.add(Effect::Regeneration, Instance::new(0, 100));

        let written = bitcode::encode(&held);
        let read: ActiveEffects = bitcode::decode(&written).expect("what was written reads back");
        assert_eq!(read, held);
    }

    #[test]
    fn milk_takes_everything_away() {
        let mut held = ActiveEffects::default();
        held.add(Effect::Speed, Instance::new(0, 100));
        held.add(Effect::Poison, Instance::new(0, 100));
        assert_eq!(held.clear().len(), 2);
        assert!(held.is_empty());
    }

    #[test]
    fn a_level_reads_as_a_level_rather_than_an_amplifier() {
        let mut held = ActiveEffects::default();
        held.add(Effect::Speed, Instance::new(0, 100));
        assert_eq!(held.level(Effect::Speed), Some(1), "amplifier 0 is Speed I");
        assert_eq!(held.level(Effect::Poison), None);
    }
}
