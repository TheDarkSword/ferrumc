use bevy_ecs::prelude::{Commands, Entity, Query, Res, ResMut};
use ferrumc_core::tick::TickCounter;
use ferrumc_core::time::{LastSentTimeUpdate, WorldTime};
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::update_time::UpdateTimePacket;
use tracing::warn;

pub fn tick_daylight_cycle(
    mut world_time: ResMut<WorldTime>,
    tick: Res<TickCounter>,
    players: Query<(Entity, &StreamWriter)>,
    mut last_sent_time: Query<&mut LastSentTimeUpdate>,
    mut commands: Commands,
) {
    world_time.advance_tick();

    let packet =
        UpdateTimePacket::overworld(tick.get() as i64, world_time.current_time() as i64, true);

    for (eid, writer) in players.iter() {
        if let Ok(mut last_sent_time_update) = last_sent_time.get_mut(eid) {
            if last_sent_time_update.should_resend() {
                last_sent_time_update.reset();

                writer.send_packet_ref(&packet).unwrap_or_else(|_| {
                    warn!("Failed to send UpdateTimePacket to player {}", eid);
                });
            }
        } else {
            commands.entity(eid).insert(LastSentTimeUpdate::default());

            writer.send_packet_ref(&packet).unwrap_or_else(|_| {
                warn!("Failed to send UpdateTimePacket to player {}", eid);
            });
        }
    }
}
