//! What an entity carries beyond its type.
//!
//! Everything the game says about a *type* lives on the type itself; these are the things that
//! differ between two entities of the same kind.

pub mod combat;
pub mod last_synced_position;
pub mod physical;

// Re-exports
pub use combat::CombatProperties;
pub use last_synced_position::LastSyncedPosition;
pub use physical::{BoundingBox, PhysicalProperties};

// Marker component for baby entities
use bevy_ecs::prelude::Component;

/// Marker component for baby entities.
/// When present, physics systems will use baby-scaled properties.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Baby;
