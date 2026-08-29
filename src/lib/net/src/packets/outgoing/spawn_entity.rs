use crate::errors::NetError;

use bevy_ecs::prelude::{Entity, Query};
use ferrumc_core::identity::entity_identity::EntityIdentity;
use ferrumc_core::identity::player_identity::PlayerIdentity;
use ferrumc_core::transform::position::Position;
use ferrumc_core::transform::rotation::Rotation;
use ferrumc_macros::{get_registry_entry, packet, NetEncode};
use ferrumc_net_codec::net_types::angle::NetAngle;
use ferrumc_net_codec::net_types::lp_vec3::LowPrecisionVec3;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::registry_remap::NetworkEntityType;

/// Spawn-time movement. Nothing spawns in motion yet, and a vector that short is a single byte.
const NOT_MOVING: LowPrecisionVec3 = LowPrecisionVec3::ZERO;

#[derive(NetEncode)]
#[packet(packet_id = "add_entity", state = "play")]
#[downgrade_with(crate::translate::to_1_21_7::add_entity)]
pub struct SpawnEntityPacket {
    pub entity_id: VarInt,
    pub entity_uuid: u128,
    pub entity_type: NetworkEntityType,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub movement: LowPrecisionVec3,
    pub pitch: NetAngle,
    pub yaw: NetAngle,
    pub head_yaw: NetAngle,
    pub data: VarInt,
}

const PLAYER_ID: u64 = get_registry_entry!("minecraft:entity_type.entries.minecraft:player");

impl SpawnEntityPacket {
    /// Creates a spawn entity packet from direct component values.
    ///
    /// This is useful when you have the component values directly
    /// rather than needing to query them.
    pub fn new(
        entity_id: i32,
        entity_uuid: u128,
        entity_type_id: i32,
        position: &Position,
        rotation: &Rotation,
    ) -> Self {
        let (x, y, z) = position.xyz();
        let (yaw, pitch) = rotation.yaw_pitch();

        Self {
            entity_id: VarInt::new(entity_id),
            entity_uuid,
            entity_type: NetworkEntityType(entity_type_id as u32),
            x,
            y,
            z,
            movement: NOT_MOVING,
            pitch: NetAngle::from_degrees(pitch as f64),
            yaw: NetAngle::from_degrees(yaw as f64),
            head_yaw: NetAngle::from_degrees(yaw as f64),
            data: VarInt::new(0),
        }
    }

    pub fn player(
        entity_id: Entity,
        query: Query<(&PlayerIdentity, &Position, &Rotation)>,
    ) -> Result<Self, NetError> {
        let (player_identity, position, rotation) = query
            .get(entity_id)
            .map_err(|e| NetError::ECSError(e.into()))?;

        let (x, y, z) = position.xyz();
        let (yaw, pitch) = rotation.yaw_pitch();

        Ok(Self {
            entity_id: VarInt::new(player_identity.short_uuid),
            entity_uuid: player_identity.uuid.as_u128(),
            entity_type: NetworkEntityType(PLAYER_ID as u32),
            x,
            y,
            z,
            movement: NOT_MOVING,
            pitch: NetAngle::from_degrees(pitch as f64),
            yaw: NetAngle::from_degrees(yaw as f64),
            head_yaw: NetAngle::from_degrees(yaw as f64),
            data: VarInt::new(0),
        })
    }

    /// Creates a spawn entity packet for any entity (mob, projectile, etc.).
    ///
    /// # Arguments
    ///
    /// * `entity` - Bevy entity to spawn
    /// * `entity_type_id` - Protocol ID for the entity type (from vanilla data)
    /// * `query` - Query to get entity components
    pub fn entity(
        entity: Entity,
        entity_type_id: u16,
        query: Query<(&EntityIdentity, &Position, &Rotation)>,
    ) -> Result<Self, NetError> {
        let (identity, position, rotation) = query
            .get(entity)
            .map_err(|e| NetError::ECSError(e.into()))?;

        let (x, y, z) = position.xyz();
        let (yaw, pitch) = rotation.yaw_pitch();

        Ok(Self {
            entity_id: VarInt::new(identity.entity_id),
            entity_uuid: identity.uuid.as_u128(),
            entity_type: NetworkEntityType(u32::from(entity_type_id)),
            x,
            y,
            z,
            movement: NOT_MOVING,
            pitch: NetAngle::from_degrees(pitch as f64),
            yaw: NetAngle::from_degrees(yaw as f64),
            head_yaw: NetAngle::from_degrees(yaw as f64),
            data: VarInt::new(0),
        })
    }
}
