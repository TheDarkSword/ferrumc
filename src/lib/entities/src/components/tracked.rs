//! What a client has been told about an entity, and how to tell it the rest.
//!
//! A moving entity is not sent outright every tick — that would be twenty-eight bytes a tick each.
//! What goes out is the change since last time, as three sixteen-bit numbers in sixteenths of a
//! thousandth of a block. That only works if both ends agree on where the entity was, and they only
//! agree if the change is worked out the way the wire carries it: rounding the old and the new
//! positions to the wire's precision and subtracting *those*, rather than subtracting first and
//! rounding after.
//!
//! Getting that backwards does not look wrong. The entity simply drifts, a fraction of a
//! thousandth of a block at a time, until it is standing somewhere it is not.

use bevy_ecs::prelude::{Component, Entity};
use bevy_math::DVec3;
use std::collections::HashSet;

/// How many steps of a block the wire carries.
const STEPS_PER_BLOCK: f64 = 4096.0;

/// The largest change one packet can carry, in those steps.
const LARGEST_CHANGE: i64 = i16::MAX as i64;

/// The smallest change worth sending, in blocks squared.
///
/// Below this the client is already close enough that a packet would say nothing.
const WORTH_SENDING: f64 = 7.629_394_5e-6;

/// How many rounds without an outright position before one is sent anyway.
///
/// Nothing should drift with the change worked out as it is here, but a client that missed a packet
/// has no other way back, so vanilla sends one every so often regardless.
const ROUNDS_BEFORE_A_TELEPORT: u32 = 400;

/// Where the wire puts a position.
fn to_wire(value: f64) -> i64 {
    (value * STEPS_PER_BLOCK).round() as i64
}

/// The angle byte the wire carries, which is a turn in two hundred and fifty-six parts.
#[must_use]
pub fn to_angle(degrees: f32) -> u8 {
    (degrees * 256.0 / 360.0).round() as i32 as u8
}

/// What a client has been told about an entity.
#[derive(Component, Debug, Clone, Default)]
pub struct Tracked {
    /// Where the client believes the entity is. Only moved when a position actually went out.
    base: DVec3,
    last_yaw: u8,
    last_pitch: u8,
    was_on_ground: bool,
    /// Rounds since a position was sent outright rather than as a change.
    since_teleport: u32,
    /// Which players have been told this entity exists.
    pub seen_by: HashSet<Entity>,
}

/// What to send a client this round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Nothing has moved or turned enough to be worth a packet.
    Nothing,
    /// How far it moved, in the steps the wire carries.
    Moved {
        x: i16,
        y: i16,
        z: i16,
    },
    Turned {
        yaw: u8,
        pitch: u8,
    },
    MovedAndTurned {
        x: i16,
        y: i16,
        z: i16,
        yaw: u8,
        pitch: u8,
    },
    /// Where it is, outright. Sent when the change is too large to carry, when it has been too long
    /// since the last one, or when it has just landed or left the ground.
    Teleported,
}

impl Tracked {
    /// A client that has just been told where an entity is.
    #[must_use]
    pub fn starting_at(position: DVec3, yaw: f32, pitch: f32, on_ground: bool) -> Self {
        Self {
            base: position,
            last_yaw: to_angle(yaw),
            last_pitch: to_angle(pitch),
            was_on_ground: on_ground,
            since_teleport: 0,
            seen_by: HashSet::new(),
        }
    }

    /// Where the client believes the entity is.
    #[must_use]
    pub const fn believed_position(&self) -> DVec3 {
        self.base
    }

    /// What to send this round, and what to remember having sent.
    ///
    /// The two are worked out together because they have to agree: a round that sends only a turn
    /// must not move the position the next change is measured from, or everything after it is
    /// measured from somewhere the client was never told about.
    #[must_use]
    pub fn change(&mut self, position: DVec3, yaw: f32, pitch: f32, on_ground: bool) -> Change {
        self.since_teleport += 1;

        let (yaw, pitch) = (to_angle(yaw), to_angle(pitch));
        let turned = yaw != self.last_yaw || pitch != self.last_pitch;

        let steps = DVec3::new(
            (to_wire(position.x) - to_wire(self.base.x)) as f64,
            (to_wire(position.y) - to_wire(self.base.y)) as f64,
            (to_wire(position.z) - to_wire(self.base.z)) as f64,
        );
        let too_far = steps
            .to_array()
            .iter()
            .any(|step| *step < -(LARGEST_CHANGE + 1) as f64 || *step > LARGEST_CHANGE as f64);
        let moved = (position - self.base).length_squared() >= WORTH_SENDING;

        if too_far
            || self.since_teleport > ROUNDS_BEFORE_A_TELEPORT
            || on_ground != self.was_on_ground
        {
            self.base = position;
            self.last_yaw = yaw;
            self.last_pitch = pitch;
            self.was_on_ground = on_ground;
            self.since_teleport = 0;
            return Change::Teleported;
        }

        let (x, y, z) = (steps.x as i16, steps.y as i16, steps.z as i16);
        match (moved, turned) {
            (false, false) => Change::Nothing,
            (true, false) => {
                self.base = position;
                Change::Moved { x, y, z }
            }
            (false, true) => {
                self.last_yaw = yaw;
                self.last_pitch = pitch;
                Change::Turned { yaw, pitch }
            }
            (true, true) => {
                self.base = position;
                self.last_yaw = yaw;
                self.last_pitch = pitch;
                Change::MovedAndTurned {
                    x,
                    y,
                    z,
                    yaw,
                    pitch,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn still() -> Tracked {
        Tracked::starting_at(DVec3::ZERO, 0.0, 0.0, true)
    }

    #[test]
    fn something_that_has_not_moved_says_nothing() {
        let mut tracked = still();
        assert_eq!(tracked.change(DVec3::ZERO, 0.0, 0.0, true), Change::Nothing);
    }

    #[test]
    fn a_step_is_carried_as_the_steps_the_wire_takes() {
        let mut tracked = still();
        assert_eq!(
            tracked.change(DVec3::new(1.0, 0.0, 0.0), 0.0, 0.0, true),
            Change::Moved {
                x: 4096,
                y: 0,
                z: 0
            }
        );
    }

    #[test]
    fn a_turn_on_its_own_leaves_the_position_where_it_was() {
        let mut tracked = still();
        // Far enough to be worth a packet on its own, but the entity has not moved.
        let turn = tracked.change(DVec3::ZERO, 90.0, 0.0, true);
        assert!(matches!(turn, Change::Turned { .. }));
        assert_eq!(tracked.believed_position(), DVec3::ZERO);
    }

    #[test]
    fn a_thousand_small_steps_leave_the_client_exactly_where_the_entity_is() {
        // The whole point of the fixed-point arithmetic. A step that does not divide evenly into
        // the wire's precision would, worked out the obvious way, lose a fraction every round, and
        // after a thousand rounds the client would be holding an entity that is somewhere else.
        let mut tracked = still();
        let step = 0.013_57_f64;
        let mut at = DVec3::ZERO;
        // What the client is holding, in the wire's own steps, which is how a client holds it.
        let mut client = 0i64;

        for _ in 0..1000 {
            at.x += step;
            match tracked.change(at, 0.0, 0.0, true) {
                Change::Moved { x, .. } | Change::MovedAndTurned { x, .. } => {
                    client += i64::from(x);
                }
                // An outright position replaces whatever the client had.
                Change::Teleported => client = to_wire(at.x),
                Change::Nothing | Change::Turned { .. } => {}
            }
        }

        assert_eq!(
            client,
            to_wire(at.x),
            "the client should be holding the entity exactly where it is"
        );
    }

    #[test]
    fn a_jump_too_large_to_carry_is_sent_outright() {
        let mut tracked = still();
        assert_eq!(
            tracked.change(DVec3::new(9.0, 0.0, 0.0), 0.0, 0.0, true),
            Change::Teleported
        );
        assert_eq!(tracked.believed_position(), DVec3::new(9.0, 0.0, 0.0));
    }

    #[test]
    fn eight_blocks_still_fits() {
        let mut tracked = still();
        assert!(matches!(
            tracked.change(DVec3::new(7.99, 0.0, 0.0), 0.0, 0.0, true),
            Change::Moved { .. }
        ));
    }

    #[test]
    fn landing_is_worth_saying_outright() {
        let mut tracked = Tracked::starting_at(DVec3::ZERO, 0.0, 0.0, false);
        assert_eq!(
            tracked.change(DVec3::ZERO, 0.0, 0.0, true),
            Change::Teleported,
            "whether it is standing on something is not carried by a change"
        );
    }

    #[test]
    fn something_standing_still_is_still_placed_now_and_then() {
        let mut tracked = still();
        let mut told = 0;
        for _ in 0..ROUNDS_BEFORE_A_TELEPORT + 2 {
            if tracked.change(DVec3::ZERO, 0.0, 0.0, true) == Change::Teleported {
                told += 1;
            }
        }
        assert_eq!(
            told, 1,
            "a client that missed a packet has no other way back"
        );
    }
}
