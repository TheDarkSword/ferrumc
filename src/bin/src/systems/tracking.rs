//! Which clients are told about which entities, and how often.
//!
//! A client is not told about everything in the world. It is told about what is near it, at a range
//! the entity's own kind decides — a painting reaches ten chunks, an ender dragon reaches ten times
//! that — and it is told again only as often as that kind is worth updating.
//!
//! What goes out between one update and the next is the change, not the position: three sixteen-bit
//! numbers rather than three doubles. The arithmetic that makes that safe is in
//! `ferrumc_entities::components::tracked`; what is here is who to send it to.

use bevy_ecs::prelude::*;
use ferrumc_core::identity::entity_identity::EntityIdentity;
use ferrumc_core::identity::player_identity::PlayerIdentity;
use ferrumc_core::tick::TickCounter;
use ferrumc_core::transform::grounded::OnGround;
use ferrumc_core::transform::position::Position;
use ferrumc_core::transform::rotation::Rotation;
use ferrumc_core::transform::velocity::Velocity;
use ferrumc_entities::components::{Change, Tracked};
use ferrumc_entities::entity_type::EntityType;
use ferrumc_entities::synced_data::SyncedData;
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::entity_metadata::EntityMetadataPacket;
use ferrumc_net::packets::outgoing::entity_position_sync::TeleportEntityPacket;
use ferrumc_net::packets::outgoing::remove_entities::RemoveEntitiesPacket;
use ferrumc_net::packets::outgoing::spawn_entity::SpawnEntityPacket;
use ferrumc_net::packets::outgoing::update_entity_position::UpdateEntityPositionPacket;
use ferrumc_net::packets::outgoing::update_entity_position_and_rotation::UpdateEntityPositionAndRotationPacket;
use ferrumc_net::packets::outgoing::update_entity_rotation::UpdateEntityRotationPacket;
use ferrumc_net_codec::net_types::angle::NetAngle;
use ferrumc_net_codec::net_types::var_int::VarInt;
use tracing::warn;

/// How many blocks a chunk of tracking range is worth.
const BLOCKS_PER_CHUNK: f64 = 16.0;

/// What is read off an entity to decide who should be told it exists.
type Appearing<'a> = (
    &'a EntityType,
    &'a EntityIdentity,
    &'a Position,
    &'a Rotation,
    &'a SyncedData,
    &'a mut Tracked,
);

/// What is read off an entity to say how it has moved.
type Tracking<'a> = (
    Entity,
    &'a EntityType,
    &'a EntityIdentity,
    &'a Position,
    &'a Rotation,
    &'a OnGround,
    &'a mut Tracked,
);

/// Tells a client about an entity that has come within range, and stops telling it about one that
/// has gone out of it.
pub fn update_who_sees_what(
    mut entities: Query<Appearing, Without<PlayerIdentity>>,
    players: Query<(Entity, &Position, &StreamWriter), With<PlayerIdentity>>,
) {
    for (kind, identity, position, rotation, data, mut tracked) in &mut entities {
        let range = f64::from(kind.tracking_range()) * BLOCKS_PER_CHUNK;
        let range_squared = range * range;

        for (player, at, writer) in &players {
            // Vanilla measures along the ground only, so an entity far below a player is still
            // near it.
            let (dx, dz) = (at.x - position.x, at.z - position.z);
            let near = dx * dx + dz * dz <= range_squared;
            let known = tracked.seen_by.contains(&player);

            if near && !known {
                let spawn = SpawnEntityPacket::new(
                    identity.entity_id,
                    identity.uuid.as_u128(),
                    i32::from(kind.protocol_id()),
                    position,
                    rotation,
                );
                let metadata =
                    EntityMetadataPacket::everything(VarInt::new(identity.entity_id), data);
                if writer.send_packet_ref(&spawn).is_ok()
                    && writer.send_packet_ref(&metadata).is_ok()
                {
                    tracked.seen_by.insert(player);
                }
            } else if !near && known {
                let gone = RemoveEntitiesPacket::of(&[identity.entity_id]);
                if let Err(err) = writer.send_packet_ref(&gone) {
                    warn!("could not tell a player an entity has gone: {err:?}");
                }
                tracked.seen_by.remove(&player);
            }
        }
    }
}

/// Tells everyone watching an entity how it has moved.
///
/// Each kind decides how often it is worth saying: a painting never moves and is never mentioned
/// again, while an arrow is worth a word every tick.
pub fn send_entity_changes(
    mut entities: Query<Tracking, Without<PlayerIdentity>>,
    velocities: Query<&Velocity>,
    writers: Query<&StreamWriter>,
    tick: Res<TickCounter>,
) {
    for (entity, kind, identity, position, rotation, grounded, mut tracked) in &mut entities {
        let Some(interval) = kind.update_interval() else {
            // A thing that never moves is placed once and not spoken of again.
            continue;
        };
        if !tick.get().is_multiple_of(u64::from(interval)) {
            continue;
        }
        if tracked.seen_by.is_empty() {
            continue;
        }

        let change = tracked.change(position.coords, rotation.yaw, rotation.pitch, grounded.0);
        if change == Change::Nothing {
            continue;
        }

        let id = VarInt::new(identity.entity_id);
        let watching = tracked
            .seen_by
            .iter()
            .filter_map(|player| writers.get(*player).ok());
        for writer in watching {
            let sent = match change {
                Change::Nothing => Ok(()),
                Change::Moved { x, y, z } => writer.send_packet_ref(&UpdateEntityPositionPacket {
                    entity_id: id,
                    delta_x: x,
                    delta_y: y,
                    delta_z: z,
                    on_ground: grounded.0,
                }),
                Change::Turned { yaw, pitch } => {
                    writer.send_packet_ref(&UpdateEntityRotationPacket {
                        entity_id: id,
                        yaw: NetAngle::new(yaw),
                        pitch: NetAngle::new(pitch),
                        on_ground: grounded.0,
                    })
                }
                Change::MovedAndTurned {
                    x,
                    y,
                    z,
                    yaw,
                    pitch,
                } => writer.send_packet_ref(&UpdateEntityPositionAndRotationPacket {
                    entity_id: id,
                    delta_x: x,
                    delta_y: y,
                    delta_z: z,
                    yaw: NetAngle::new(yaw),
                    pitch: NetAngle::new(pitch),
                    on_ground: grounded.0,
                }),
                Change::Teleported => {
                    let moving = velocities.get(entity).map_or([0.0; 3], |v| {
                        [f64::from(v.x), f64::from(v.y), f64::from(v.z)]
                    });
                    writer.send_packet_ref(&TeleportEntityPacket {
                        entity_id: id,
                        x: position.x,
                        y: position.y,
                        z: position.z,
                        vel_x: moving[0],
                        vel_y: moving[1],
                        vel_z: moving[2],
                        yaw: rotation.yaw,
                        pitch: rotation.pitch,
                        on_ground: grounded.0,
                    })
                }
            };
            if let Err(err) = sent {
                warn!("could not tell a player how an entity moved: {err:?}");
            }
        }
    }
}

/// Stops a player being counted as watching anything once they have gone.
///
/// Without this an entity keeps a name that no longer answers, and every round spends time looking
/// for a connection that is not there.
pub fn forget_players_who_left(
    mut entities: Query<&mut Tracked>,
    players: Query<Entity, With<PlayerIdentity>>,
) {
    let here: std::collections::HashSet<Entity> = players.iter().collect();
    for mut tracked in &mut entities {
        if tracked.seen_by.iter().any(|player| !here.contains(player)) {
            tracked.seen_by.retain(|player| here.contains(player));
        }
    }
}
