use bevy_ecs::prelude::{Entity, Message};
use ferrumc_core::transform::position::Position;
use ferrumc_entities::entity_type::EntityType;

/// Asks to spawn an entity in front of a player.
///
/// This message is written by the /spawn command and processed by
/// the spawn_command_processor system which calculates the spawn position.
#[derive(Message)]
pub struct SpawnEntityCommand {
    pub entity_type: EntityType,
    pub player_entity: Entity,
}

/// Event fired when an entity should be spawned at a specific position.
///
/// This is triggered by spawn_command_processor after calculating
/// the spawn position from the player's position and rotation.
#[derive(Message)]
pub struct SpawnEntityEvent {
    pub entity_type: EntityType,
    pub position: Position,
    /// The name it already had, for one that is coming back rather than appearing. A new entity
    /// carries nothing here and is given a name of its own.
    pub uuid: Option<uuid::Uuid>,
}

impl SpawnEntityEvent {
    /// A new entity, which has never existed before.
    #[must_use]
    pub const fn fresh(entity_type: EntityType, position: Position) -> Self {
        Self {
            entity_type,
            position,
            uuid: None,
        }
    }
}
