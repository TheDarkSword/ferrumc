//! Mobs: putting them in the world, and what they do once there.
//!
//! Behaviour is Phase 7's; what is here is only how one comes to exist and how it stops.

use bevy_ecs::schedule::IntoScheduleConfigs;

pub mod natural;
mod pig;

pub fn register_mob_systems(schedule: &mut bevy_ecs::schedule::Schedule) {
    // Taking mobs away comes after putting them down, so one that was just put somewhere nobody is
    // near does not survive the tick it appeared in.
    schedule.add_systems((natural::spawn_mobs, natural::despawn_mobs).chain());
}
