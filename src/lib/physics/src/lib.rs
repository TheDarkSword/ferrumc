//! What a tick does to an entity's velocity.
//!
//! Two things move an entity on its own: the pull downwards and what the medium around it takes
//! back. Both are small numbers, and the order they are applied in is what decides how fast a
//! thing ends up falling — apply the drag to the velocity before the pull rather than after and
//! the terminal speed comes out wrong while everything still looks roughly right.
//!
//! Vanilla runs them in two different orders. A mob moves with whatever the last tick left it and
//! is pulled down and slowed afterwards; a dropped thing is pulled down first, moves, and is slowed
//! afterwards — so a dropped thing is always one tick ahead of a mob dropped beside it.
//! [`before_move`] and [`after_move`] are the two halves, and which of them does the work for a
//! given entity is [`Motion::living`]. Getting this backwards costs a mob about two blocks over the
//! first second of a fall while everything still looks like falling.
//!
//! Nothing here touches the world. Whether an entity is standing on something, and on what, and
//! whether it is in a fluid, are answered by the caller and passed in.

use bevy_math::Vec3A;

/// What a tick takes off a mob's horizontal speed in the air, before the block underneath is
/// taken into account.
///
/// Vanilla applies this one to a living entity and its own air drag to everything else, which is
/// why a mob slides further along ice than a dropped item does.
const LIVING_HORIZONTAL_DRAG: f32 = 0.91;

/// What is left of an entity's vertical speed after a tick in water, and of a mob's horizontal
/// speed with it.
const WATER_DRAG: f32 = 0.8;

/// What is left of a mob's speed after a tick in lava.
const LAVA_DRAG: f32 = 0.5;

/// How much a dropped thing rises per tick while it is under water or lava, up to the speed it
/// stops rising at.
const FLOAT_RISE: f32 = 0.000_5;

/// The speed above which a dropped thing stops being pushed up.
const FLOAT_CEILING: f32 = 0.06;

/// What is left of a dropped thing's downward speed when it hits the ground: it bounces, a little.
const LANDING_BOUNCE: f32 = -0.5;

/// The friction of a block that has nothing unusual about it.
///
/// Ice and slime and honey differ; nothing reads them yet, so everything is this.
pub const DEFAULT_BLOCK_FRICTION: f32 = 0.6;

/// How an entity of some kind is moved, as the game answers it.
///
/// Every field is the game's own answer for that type rather than a constant that happens to fit
/// most of them: an item is pulled down at half the rate a mob is, an arrow at rather more than
/// half, and twenty-three kinds are not pulled down at all.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    /// How much downward speed a tick adds.
    pub gravity: f32,
    /// What is left of the vertical speed after a tick of air.
    pub air_drag: f32,
    /// How tall a rise it walks up rather than into.
    pub step_height: f32,
    /// Whether it moves the way a mob moves rather than the way a dropped thing does.
    pub living: bool,
    /// Whether the air holds it back in every direction equally, which is true of the few things
    /// that fly under their own power.
    pub omnidirectional: bool,
    /// Whether a current carries it along.
    pub pushed_by_fluid: bool,
}

/// The medium an entity is standing in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fluid {
    Water,
    Lava,
}

/// What an entity is standing on, if anything.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Footing {
    /// In the air, or falling.
    None,
    /// On a block of this friction.
    On(f32),
}

impl Footing {
    /// How much of an entity's horizontal speed the ground leaves it. Air leaves all of it.
    const fn friction(self) -> f32 {
        match self {
            Self::None => 1.0,
            Self::On(friction) => friction,
        }
    }

    const fn grounded(self) -> bool {
        matches!(self, Self::On(_))
    }
}

/// The velocity an entity moves with this tick.
///
/// A mob moves with what the last tick left it: nothing happens to it here. A dropped thing is
/// pulled down first, or carried by the fluid it is in.
#[must_use]
pub fn before_move(velocity: Vec3A, motion: &Motion, fluid: Option<Fluid>) -> Vec3A {
    if motion.living {
        return velocity;
    }
    match fluid {
        None => velocity - Vec3A::Y * motion.gravity,
        Some(fluid) => floating(velocity, fluid),
    }
}

/// What the tick leaves an entity with once it has moved.
///
/// A mob is pulled down and slowed here, which is why it is a tick behind a dropped thing falling
/// beside it. A dropped thing only has its drag taken, and loses most of its downward speed if it
/// has just landed.
#[must_use]
pub fn after_move(
    velocity: Vec3A,
    motion: &Motion,
    footing: Footing,
    fluid: Option<Fluid>,
) -> Vec3A {
    if motion.living {
        return match fluid {
            None => living_in_air(velocity, motion, footing),
            Some(fluid) => living_in_fluid(velocity, motion, fluid),
        };
    }

    let horizontal = motion.air_drag * footing.friction();
    let mut moved = velocity * Vec3A::new(horizontal, motion.air_drag, horizontal);
    if footing.grounded() && moved.y < 0.0 {
        moved.y *= LANDING_BOUNCE;
    }
    moved
}

/// A mob falling through air: pulled down, then slowed, with the ground it is on holding back its
/// horizontal speed as well as the air does.
fn living_in_air(velocity: Vec3A, motion: &Motion, footing: Footing) -> Vec3A {
    // The vertical drag is the type's own — the few things that fly under their own power are
    // held back as much going up as going along, and the game already answered which those are.
    let horizontal = footing.friction() * LIVING_HORIZONTAL_DRAG;
    Vec3A::new(
        velocity.x * horizontal,
        (velocity.y - motion.gravity) * motion.air_drag,
        velocity.z * horizontal,
    )
}

/// A mob in water or lava, which is slowed far more than the air slows it and sinks rather than
/// falls.
fn living_in_fluid(velocity: Vec3A, motion: &Motion, fluid: Fluid) -> Vec3A {
    let falling = velocity.y <= 0.0;
    let slowed = match fluid {
        Fluid::Water => velocity * Vec3A::new(WATER_DRAG, WATER_DRAG, WATER_DRAG),
        Fluid::Lava => velocity * Vec3A::new(LAVA_DRAG, WATER_DRAG, LAVA_DRAG),
    };
    let adjusted = sinking(slowed, motion.gravity, falling);
    match fluid {
        Fluid::Water => adjusted,
        // Lava pulls harder than it drags, on top of what the drag already took.
        Fluid::Lava => adjusted - Vec3A::Y * (motion.gravity / 4.0),
    }
}

/// How fast something sinks rather than falls.
///
/// A thing already on its way down that would end up almost exactly at the sinking speed is held
/// just under it instead, which is what keeps a mob from hovering at the bottom of a pool.
fn sinking(velocity: Vec3A, gravity: f32, falling: bool) -> Vec3A {
    if gravity == 0.0 {
        return velocity;
    }
    let sink = gravity / 16.0;
    let y = if falling && (velocity.y - 0.005).abs() >= 0.003 && (velocity.y - sink).abs() < 0.003 {
        -0.003
    } else {
        velocity.y - sink
    };
    Vec3A::new(velocity.x, y, velocity.z)
}

/// A dropped thing in a fluid, which drifts to a stop sideways and rises slowly until it is at the
/// surface.
fn floating(velocity: Vec3A, fluid: Fluid) -> Vec3A {
    let drag = match fluid {
        Fluid::Water => 0.99,
        Fluid::Lava => 0.95,
    };
    let rise = if velocity.y < FLOAT_CEILING {
        FLOAT_RISE
    } else {
        0.0
    };
    Vec3A::new(velocity.x * drag, velocity.y + rise, velocity.z * drag)
}

/// The most vertical speed a knock can give something that is standing on the ground.
const KNOCKBACK_LIFT_CEILING: f32 = 0.4;

/// The velocity something is left with after being knocked away from `from`.
///
/// Half of what it already had, plus the push. Something standing on the ground is lifted as well,
/// up to a ceiling, which is what makes a hit knock a mob up and back rather than only back; one
/// already in the air keeps the fall it was in.
///
/// `resistance` is how much of the push the target shrugs off, from nothing to all of it. A push
/// with no direction at all does nothing, rather than sending the target off along an axis it was
/// never pushed along.
#[must_use]
pub fn knockback(
    velocity: Vec3A,
    power: f32,
    from: bevy_math::Vec2,
    resistance: f32,
    footing: Footing,
) -> Vec3A {
    let power = power * (1.0 - resistance);
    if power <= 0.0 || from.length_squared() < 1.0e-5 {
        return velocity;
    }

    let push = from.normalize() * power;
    let lifted = if footing.grounded() {
        (velocity.y / 2.0 + power).min(KNOCKBACK_LIFT_CEILING)
    } else {
        velocity.y
    };
    Vec3A::new(velocity.x / 2.0 - push.x, lifted, velocity.z / 2.0 - push.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOB: Motion = Motion {
        gravity: 0.08,
        air_drag: 0.98,
        step_height: 0.6,
        living: true,
        omnidirectional: false,
        pushed_by_fluid: true,
    };

    const ITEM: Motion = Motion {
        gravity: 0.04,
        air_drag: 0.98,
        step_height: 0.0,
        living: false,
        omnidirectional: false,
        pushed_by_fluid: true,
    };

    /// Where a velocity settles when a tick stops changing it.
    fn terminal(motion: &Motion) -> f32 {
        fall(motion, 2000).1
    }

    /// How far something falls from rest in `ticks`, and how fast it is going by then.
    fn fall(motion: &Motion, ticks: usize) -> (f32, f32) {
        let mut velocity = Vec3A::ZERO;
        let mut fallen = 0.0;
        for _ in 0..ticks {
            velocity = before_move(velocity, motion, None);
            fallen -= velocity.y;
            velocity = after_move(velocity, motion, Footing::None, None);
        }
        (fallen, velocity.y)
    }

    #[test]
    fn a_falling_mob_settles_where_vanilla_settles_it() {
        // The pull and the drag balance at -gravity * drag / (1 - drag), which for a mob is the
        // number every fall-damage table is written against.
        assert!((terminal(&MOB) - -3.92).abs() < 1e-4, "{}", terminal(&MOB));
    }

    #[test]
    fn a_dropped_thing_falls_slower_than_a_mob() {
        assert!(
            terminal(&ITEM) > terminal(&MOB),
            "an item is pulled down at half the rate"
        );
        assert!(
            (terminal(&ITEM) - -1.96).abs() < 1e-4,
            "{}",
            terminal(&ITEM)
        );
    }

    #[test]
    fn the_two_orders_are_a_tick_apart() {
        // Same pull, same drag, same settling speed — but a mob spends its first tick standing
        // still while a dropped thing has already been pulled down, and never makes it back.
        let heavy_item = Motion {
            gravity: MOB.gravity,
            living: false,
            ..ITEM
        };
        assert_eq!(
            fall(&MOB, 1).0,
            0.0,
            "a mob moves with what it had, which was nothing"
        );
        assert_eq!(fall(&heavy_item, 1).0, MOB.gravity);
    }

    #[test]
    fn a_fall_covers_the_ground_vanilla_covers() {
        // Twenty ticks is one second: long enough for a wrong order to show, short enough to still
        // be accelerating. These are the game's own numbers, not this code's.
        let (mob, _) = fall(&MOB, 20);
        let (item, _) = fall(&ITEM, 20);
        assert!((mob - 13.2512).abs() < 1e-3, "a mob fell {mob}");
        assert!((item - 7.4256).abs() < 1e-3, "an item fell {item}");
    }

    #[test]
    fn nothing_pulls_on_something_that_is_not_pulled_on() {
        let squid = Motion {
            gravity: 0.0,
            ..MOB
        };
        let moving = Vec3A::new(0.0, 0.5, 0.0);
        assert_eq!(
            after_move(moving, &squid, Footing::None, None).y,
            0.5 * squid.air_drag
        );
    }

    #[test]
    fn a_dropped_thing_that_lands_keeps_a_little_of_its_fall() {
        let landed = after_move(Vec3A::new(0.0, -1.0, 0.0), &ITEM, Footing::On(0.6), None);
        assert_eq!(landed.y, -(ITEM.air_drag * LANDING_BOUNCE));
    }

    #[test]
    fn the_ground_holds_a_mob_back_more_than_the_air_does() {
        let sliding = Vec3A::new(1.0, 0.0, 0.0);
        let in_air = after_move(sliding, &MOB, Footing::None, None).x;
        let on_ground = after_move(sliding, &MOB, Footing::On(DEFAULT_BLOCK_FRICTION), None).x;
        assert!(on_ground < in_air);
        assert_eq!(in_air, LIVING_HORIZONTAL_DRAG);
        assert_eq!(on_ground, DEFAULT_BLOCK_FRICTION * LIVING_HORIZONTAL_DRAG);
    }

    #[test]
    fn a_dropped_thing_in_water_rises_to_the_surface() {
        let mut velocity = Vec3A::new(0.0, -0.5, 0.0);
        for _ in 0..200 {
            velocity = before_move(velocity, &ITEM, Some(Fluid::Water));
            velocity = after_move(velocity, &ITEM, Footing::None, Some(Fluid::Water));
        }
        assert!(velocity.y > 0.0, "it should be on its way up, not down");
        assert!(
            velocity.y < FLOAT_CEILING,
            "and it should stop rising once it is drifting"
        );
    }

    #[test]
    fn a_knock_sends_a_mob_away_from_what_hit_it_and_up() {
        let hit = knockback(
            Vec3A::ZERO,
            0.4,
            bevy_math::Vec2::new(1.0, 0.0),
            0.0,
            Footing::On(DEFAULT_BLOCK_FRICTION),
        );
        assert_eq!(hit.x, -0.4, "away from the hit, not towards it");
        assert_eq!(hit.y, KNOCKBACK_LIFT_CEILING, "and up, as far as it goes");
        assert_eq!(hit.z, 0.0);
    }

    #[test]
    fn a_knock_in_the_air_does_not_lift() {
        let falling = Vec3A::new(0.0, -1.0, 0.0);
        let hit = knockback(
            falling,
            0.4,
            bevy_math::Vec2::new(1.0, 0.0),
            0.0,
            Footing::None,
        );
        assert_eq!(hit.y, -1.0, "it keeps the fall it was already in");
    }

    #[test]
    fn a_knock_something_shrugs_off_does_nothing() {
        let moving = Vec3A::new(1.0, 0.0, 0.0);
        assert_eq!(
            knockback(
                moving,
                0.4,
                bevy_math::Vec2::new(1.0, 0.0),
                1.0,
                Footing::None
            ),
            moving
        );
    }

    #[test]
    fn a_knock_from_nowhere_does_nothing() {
        let moving = Vec3A::new(1.0, 0.0, 0.0);
        assert_eq!(
            knockback(moving, 0.4, bevy_math::Vec2::ZERO, 0.0, Footing::None),
            moving
        );
    }

    #[test]
    fn a_mob_sinks_rather_than_falls() {
        let sinking = after_move(Vec3A::ZERO, &MOB, Footing::None, Some(Fluid::Water)).y;
        let falling = after_move(Vec3A::ZERO, &MOB, Footing::None, None).y;
        assert!(sinking > falling, "water holds it up");
        assert_eq!(sinking, -MOB.gravity / 16.0);
    }
}
