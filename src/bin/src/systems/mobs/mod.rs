//! Mobs: putting them in the world, and what they do once there.
//!
//! Behaviour is Phase 7's; what is here is only how one comes to exist and how it stops.

use bevy_ecs::schedule::IntoScheduleConfigs;

pub mod natural;
pub mod persistence;
mod pig;

pub fn register_mob_systems(schedule: &mut bevy_ecs::schedule::Schedule) {
    // Taking mobs away comes after putting them down, so one that was just put somewhere nobody is
    // near does not survive the tick it appeared in.
    // A chunk brings its entities back before anything tries to put more in it, and lets go of
    // them last, so a mob that has just been put down is not written out in the same tick.
    schedule.add_systems(
        (
            persistence::load_entities_for_new_chunks,
            natural::spawn_mobs,
            natural::despawn_mobs,
            persistence::unload_entities_for_gone_chunks,
        )
            .chain(),
    );
}
