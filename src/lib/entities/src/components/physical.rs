use bevy_ecs::prelude::Component;
use bevy_math::bounding::Aabb3d;
use std::ops::{Deref, DerefMut};

/// Entity bounding box (collision box).
///
/// Represents the volume occupied by an entity in the world.
/// Used for collision detection and physics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    aabb: Aabb3d,
}

impl Deref for BoundingBox {
    type Target = Aabb3d;

    fn deref(&self) -> &Self::Target {
        &self.aabb
    }
}

impl DerefMut for BoundingBox {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.aabb
    }
}

impl BoundingBox {
    /// Creates a new bounding box from vanilla dimensions.
    ///
    /// # Arguments
    ///
    /// * `width` - how wide it is; an entity's box is square in plan
    /// * `height` - how tall it is
    pub const fn of(width: f32, height: f32) -> Self {
        Self {
            aabb: Aabb3d {
                max: bevy_math::Vec3A::new(width / 2.0, height, width / 2.0),
                min: bevy_math::Vec3A::new(-(width / 2.0), 0.0, -(width / 2.0)),
            },
        }
    }

    /// Returns the total width of the bounding box.
    pub fn width(&self) -> f64 {
        (self.aabb.max.x - self.aabb.min.x) as f64
    }

    /// Returns the height of the bounding box.
    pub fn height(&self) -> f64 {
        (self.aabb.max.y - self.aabb.min.y) as f64
    }

    /// Returns the depth of the bounding box.
    pub fn depth(&self) -> f64 {
        (self.aabb.max.z - self.aabb.min.z) as f64
    }

    /// Returns the volume of the bounding box in cubic blocks.
    pub fn volume(&self) -> f64 {
        self.width() * self.height() * self.depth()
    }
}

/// Physical properties of an entity.
///
/// These properties are derived from vanilla data but can be modified
/// by gameplay effects (baby, crouching, potions, etc.).
///
/// # Examples
///
/// ```
/// use ferrumc_entities::entity_type::EntityType;
///
/// let physical = EntityType::Pig.physical(false);
///
/// assert!((physical.bounding_box.height() - 0.9).abs() < 1e-6);
/// assert!(!physical.fire_immune);
/// ```
#[derive(Component, Clone, Copy)]
pub struct PhysicalProperties {
    /// Bounding box of the entity for collisions.
    ///
    /// Can change if the entity is a baby, crouching, etc.
    pub bounding_box: BoundingBox,

    /// Eye height in blocks from the entity's feet.
    ///
    /// Used for vision calculation, raytracing, and camera position
    /// for players.
    pub eye_height: f32,

    /// True if the entity is immune to fire and lava.
    pub fire_immune: bool,
}

impl PhysicalProperties {
    /// Applies a scale factor to dimensions (for babies for example).
    ///
    /// # Arguments
    ///
    /// * `scale` - Multiplier factor (0.5 for baby, 1.0 for adult)
    pub fn apply_scale(&mut self, scale: f64) {
        let width = self.bounding_box.width() * scale;
        let height = self.bounding_box.height() * scale;
        let depth = self.bounding_box.depth() * scale;
        self.bounding_box = BoundingBox {
            aabb: Aabb3d {
                min: bevy_math::Vec3A::new(-(width as f32) / 2.0, 0.0, -(depth as f32) / 2.0),
                max: bevy_math::Vec3A::new(
                    (width as f32) / 2.0,
                    height as f32,
                    (depth as f32) / 2.0,
                ),
            },
        };
        self.eye_height = (self.eye_height as f64 * scale) as f32;
    }
}

impl std::fmt::Debug for PhysicalProperties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhysicalProperties")
            .field("bounding_box", &self.bounding_box)
            .field("eye_height", &self.eye_height)
            .field("fire_immune", &self.fire_immune)
            .finish()
    }
}
