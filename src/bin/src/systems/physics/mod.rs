use bevy_ecs::schedule::IntoScheduleConfigs;
pub mod collisions;
pub mod drag;
pub mod gravity;
pub mod unground;
pub mod velocity;

pub fn register_physics(schedule: &mut bevy_ecs::schedule::Schedule) {
    schedule.add_systems(
        (
            unground::handle,
            gravity::handle,
            drag::handle,
            // Collision shortens the movement; velocity is what then applies it.
            collisions::handle,
            velocity::handle,
        )
            .chain(),
    );
}
