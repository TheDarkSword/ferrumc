//! Keeping what a client is told about an entity in step with what the entity is.
//!
//! Systems write to an entity's [`SyncedData`] as they go, without caring who is watching. Two
//! systems here close the loop at the end of the tick: one works out the pose the written state
//! implies, and one sends whatever ended up changed.

use bevy_ecs::prelude::*;
use ferrumc_components::health::Health;
use ferrumc_core::identity::entity_identity::EntityIdentity;
use ferrumc_core::identity::player_identity::PlayerIdentity;
use ferrumc_entities::synced_data::{fields, EntityFlag, Pose, SyncedData};
use ferrumc_net::broadcast::broadcast_packet_except;
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::entity_metadata::EntityMetadataPacket;
use ferrumc_net_codec::net_types::var_int::VarInt;

/// Copies what the server knows about an entity into what a client is told about it.
///
/// Health is kept as its own component because that is what the rest of the server reads; a client
/// reads it out of the row instead, so the two are put in step here rather than at every place
/// something takes damage.
pub fn mirror_components(mut entities: Query<(&Health, &mut SyncedData), Changed<Health>>) {
    for (health, mut data) in &mut entities {
        data.set(fields::living_entity::HEALTH, health.current);
    }
}

/// Works out the pose an entity's state implies.
///
/// Vanilla picks the first of sleeping, swimming, flying with elytra, spin attack and crouching
/// that applies, and stands otherwise. Only some of those are tracked so far, and the check that
/// the pose actually fits where the entity is standing needs collision, so what is here is that
/// order as far as it can be followed.
pub fn update_poses(mut entities: Query<&mut SyncedData, Changed<SyncedData>>) {
    for mut data in &mut entities {
        let pose = if data.flag(EntityFlag::Swimming) {
            Pose::Swimming
        } else if data.flag(EntityFlag::FallFlying) {
            Pose::FallFlying
        } else if data.flag(EntityFlag::Crouching) {
            Pose::Crouching
        } else {
            Pose::Standing
        };
        data.set(fields::entity::POSE, pose);
    }
}

/// Sends what changed to everyone but the entity it changed on.
///
/// A player is told about their own crouching by their own client, and telling them again is how a
/// client ends up fighting itself over its own pose.
pub fn broadcast_changes(
    mut entities: Query<(
        Entity,
        &mut SyncedData,
        Option<&PlayerIdentity>,
        Option<&EntityIdentity>,
    )>,
    connections: Query<(Entity, &StreamWriter)>,
) {
    for (entity, mut data, player, other) in &mut entities {
        // Read through the handle rather than write through it: touching it would mark the entity
        // changed again, and every entity would look changed on every tick.
        if !data.has_changes() {
            continue;
        }
        let network_id = match (player, other) {
            (Some(player), _) => player.short_uuid,
            (None, Some(other)) => other.entity_id,
            (None, None) => continue,
        };

        if let Some(packet) = EntityMetadataPacket::changes(VarInt::new(network_id), &data) {
            broadcast_packet_except(entity, &packet, connections.iter());
        }
        data.take_changes();
    }
}
