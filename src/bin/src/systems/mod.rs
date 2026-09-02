use bevy_ecs::schedule::IntoScheduleConfigs;
pub mod advancements;
pub mod attributes;
pub mod block_ticks;
pub mod block_world;
mod chunk_calculator;
pub mod chunk_levels;
mod chunk_sending;
pub mod chunk_unloader;
pub mod connection_killer;
pub mod damage;
pub mod datapacks;
pub mod day_cycle;
pub mod drops;
pub mod effects;
pub mod emit_player_joined;
pub mod fluids;
pub mod keep_alive_system;
pub mod lan_pinger;
pub mod listeners;
pub mod mobs;
mod mq;
pub mod new_connections;
mod particles;
pub mod physics;
mod player_swimming;
pub mod shutdown_systems;
pub mod synced_data;
pub mod tick_counter;
pub mod tps_broadcast;
pub mod tracking;
pub(crate) mod update_player_ping;
pub mod world_sync;

pub fn register_game_systems(schedule: &mut bevy_ecs::schedule::Schedule) {
    // Tick-bound systems only (run every game tick)
    // NOTE: accept_new_connections is registered separately in game_loop.rs
    // with apply_deferred and emit_player_joined chained after it.
    schedule.add_systems(
        (
            chunk_calculator::handle,
            chunk_sending::handle,
            // chunk_unloader::handle,
        )
            .chain(),
    );
    schedule.add_systems(mq::process);

    // Tell a player what they have done when they join, and again as they do more.
    schedule.add_systems(advancements::on_join);
    schedule.add_systems(advancements::on_inventory_change);
    // What a client is told about an entity: the systems above write to it as they go, then the
    // pose is worked out from what they wrote and whatever ended up changed is sent, once.
    schedule.add_systems(
        (
            player_swimming::detect_player_swimming,
            synced_data::mirror_components,
            synced_data::update_poses,
            synced_data::broadcast_changes,
        )
            .chain(),
    );

    // Process scheduled fluid ticks: evaluate spreading, apply, broadcast, re-schedule.
    schedule.add_systems(fluids::seed_on_block_break);
    // Settle generated "hanging" fluids in newly loaded chunks (cave-breached oceans, perched
    // springs) the first time a player is near, mirroring vanilla's settle-on-load.
    schedule.add_systems(fluids::settle_loaded_fluids);
    schedule.add_systems(fluids::process_fluid_ticks);

    // Who is told about what, and how it has moved since they were last told. A player who has
    // left is forgotten first, so nothing spends the round looking for a connection that is gone.
    schedule.add_systems(
        (
            tracking::forget_players_who_left,
            tracking::update_who_sees_what,
            tracking::send_entity_changes,
        )
            .chain(),
    );

    // What a broken block leaves behind, and what becomes of it. Ageing comes after picking up, so
    // something taken on the tick it would have expired is taken rather than lost.
    schedule.add_systems(
        (
            drops::drop_what_a_block_left,
            drops::pull_orbs_to_players,
            drops::pick_up_what_is_walked_over,
            drops::merge_what_is_lying_about,
            drops::age_what_is_lying_about,
        )
            .chain(),
    );

    // What is worn decides what the numbers are, and the numbers decide what a blow comes to and
    // what a swing is worth — so this runs before either, and what changed is sent after.
    schedule.add_systems(
        (
            attributes::apply_what_is_worn,
            attributes::follow_max_health,
            attributes::send_changed_attributes,
        )
            .chain(),
    );

    // What anyone is under the influence of, ticked before the damage it may cause is settled.
    // The modifiers go on before a client is told, so what is sent is what is actually in force.
    schedule.add_systems(
        (
            effects::tick_effects,
            effects::apply_effect_modifiers,
            effects::send_effect_changes,
        )
            .chain(),
    );

    // What the world does to whatever is standing in it. The blows are raised first and settled
    // after, so a tick's worth of them is worked out against one set of invulnerability frames
    // rather than against whatever order the systems happened to run in.
    schedule.add_systems(
        (
            damage::hurt_by_the_world,
            damage::apply_damage,
            // What a killed thing leaves behind is read off it while it is still there, so this
            // comes before the death that takes it out of the world.
            drops::drop_what_a_mob_left,
            damage::something_died,
            damage::tick_reeling,
        )
            .chain(),
    );

    schedule.add_systems(day_cycle::tick_daylight_cycle);

    // Stream the measured server TPS to clients (shown on F3) once per second.
    schedule.add_systems(tps_broadcast::broadcast_tps);

    // Should always be last
    schedule.add_systems(connection_killer::connection_killer);
    schedule.add_systems(particles::handle);
}
pub mod world_light;
