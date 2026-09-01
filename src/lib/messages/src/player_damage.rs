use bevy_ecs::prelude::{Entity, Message};
use ferrumc_data::generated::damage_types::DamageType;

/// Something is to be hurt.
///
/// * Fired by: the world (falling, drowning, burning, the void), and combat.
/// * Listened for by: the system that puts it through the damage pipeline and takes the health off.
///
/// The amount here is what the blow is worth before anything softens it. What actually lands is
/// worked out where it is applied, since that is where the armour and the invulnerability frames
/// are.
#[derive(Message)]
pub struct EntityDamaged {
    pub entity: Entity,
    pub kind: DamageType,
    pub amount: f32,
    /// Whoever is to blame, where anything is.
    pub cause: Option<Entity>,
}

impl EntityDamaged {
    /// A blow from the world itself, which nothing is to blame for.
    #[must_use]
    pub const fn from_the_world(entity: Entity, kind: DamageType, amount: f32) -> Self {
        Self {
            entity,
            kind,
            amount,
            cause: None,
        }
    }
}

/// Something's health has reached nothing.
///
/// * Fired by: the system that applies damage.
/// * Listened for by: respawning, death messages, and whatever a thing leaves behind.
#[derive(Message)]
pub struct EntityDied {
    pub entity: Entity,
    /// What finished it, which is what a death message is written from.
    pub kind: DamageType,
    pub cause: Option<Entity>,
}
