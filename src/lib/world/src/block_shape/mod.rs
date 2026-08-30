//! The space a block state actually occupies.
//!
//! A block is not a cube. A slab collides at half height, a fence collides half a block taller
//! than it renders so a player cannot jump it, and a carpet stops nothing at all. Treating them
//! alike is what drops a player through a trapdoor or lets one walk over a fence.
//!
//! A shape here is a list of boxes, not the bitmap over per-axis coordinates vanilla keeps: block
//! states average 1.85 boxes and reach fifteen at the worst, and at that size the bitmap costs more
//! than it saves. The tables come from the game itself; see `scripts/extract_block_shapes.py`.

pub mod generated;

use crate::block_state::Axis;
use crate::block_state_id::BlockStateId;
use generated::{BOXES, SHAPES};

/// Which shape each state occupies, as indices into [`SHAPES`].
static COLLISION: &[u8] = include_bytes!("../../../../../assets/data/block_shapes/collision.bin");
static OUTLINE: &[u8] = include_bytes!("../../../../../assets/data/block_shapes/outline.bin");

/// How close two faces have to be before they count as touching rather than overlapping. Vanilla
/// uses the same figure, and without one a shape resting exactly on another reads as inside it.
const EPSILON: f64 = 1.0E-7;

/// A box, in coordinates relative to the block that owns it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min_x: f64,
    pub min_y: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub max_z: f64,
}

impl Aabb {
    #[must_use]
    pub const fn new(
        min_x: f64,
        min_y: f64,
        min_z: f64,
        max_x: f64,
        max_y: f64,
        max_z: f64,
    ) -> Self {
        Self {
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
        }
    }

    /// The same box somewhere else, which is how a block-relative shape becomes a world one.
    #[must_use]
    pub fn offset(self, x: f64, y: f64, z: f64) -> Self {
        Self::new(
            self.min_x + x,
            self.min_y + y,
            self.min_z + z,
            self.max_x + x,
            self.max_y + y,
            self.max_z + z,
        )
    }

    #[must_use]
    pub fn min(&self, axis: Axis) -> f64 {
        match axis {
            Axis::X => self.min_x,
            Axis::Y => self.min_y,
            Axis::Z => self.min_z,
        }
    }

    #[must_use]
    pub fn max(&self, axis: Axis) -> f64 {
        match axis {
            Axis::X => self.max_x,
            Axis::Y => self.max_y,
            Axis::Z => self.max_z,
        }
    }

    /// Whether the two overlap, touching faces excluded.
    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        self.min_x < other.max_x
            && self.max_x > other.min_x
            && self.min_y < other.max_y
            && self.max_y > other.min_y
            && self.min_z < other.max_z
            && self.max_z > other.min_z
    }

    /// How far this box may still move along `axis` before it would pass into `other`.
    ///
    /// Returns `movement` unchanged when the two never meet, and a shorter distance of the same
    /// sign when they do. Movement is resolved one axis at a time, so the other two are asked only
    /// whether they overlap at all.
    #[must_use]
    pub fn collide(&self, axis: Axis, other: &Self, movement: f64) -> f64 {
        if movement == 0.0 {
            return movement;
        }
        for other_axis in [Axis::X, Axis::Y, Axis::Z] {
            if other_axis == axis {
                continue;
            }
            if self.max(other_axis) <= other.min(other_axis) + EPSILON
                || self.min(other_axis) >= other.max(other_axis) - EPSILON
            {
                return movement;
            }
        }

        if movement > 0.0 && self.max(axis) <= other.min(axis) + EPSILON {
            return movement.min(other.min(axis) - self.max(axis));
        }
        if movement < 0.0 && self.min(axis) >= other.max(axis) - EPSILON {
            return movement.max(other.max(axis) - self.min(axis));
        }
        movement
    }
}

/// The boxes a block state occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelShape(&'static [u16]);

fn shape_at(table: &'static [u8], state: BlockStateId) -> VoxelShape {
    let offset = state.raw() as usize * 2;
    let index = match table.get(offset..offset + 2) {
        Some([low, high]) => u16::from_le_bytes([*low, *high]) as usize,
        // An id this server does not generate. Nothing is safer to say about it than nothing.
        _ => return VoxelShape(&[]),
    };
    VoxelShape(SHAPES[index])
}

impl VoxelShape {
    /// What stops an entity moving through the block.
    #[must_use]
    pub fn collision_of(state: BlockStateId) -> Self {
        shape_at(COLLISION, state)
    }

    /// What the client draws a selection box around, which is not always what collides: a fence
    /// collides taller than it is drawn.
    #[must_use]
    pub fn outline_of(state: BlockStateId) -> Self {
        shape_at(OUTLINE, state)
    }

    #[must_use]
    pub fn is_empty(self) -> bool {
        self.0.is_empty()
    }

    /// The boxes, in coordinates relative to the block.
    pub fn boxes(self) -> impl Iterator<Item = Aabb> {
        self.0.iter().map(|&index| BOXES[index as usize])
    }

    /// The boxes, placed at a block position.
    pub fn boxes_at(self, x: f64, y: f64, z: f64) -> impl Iterator<Item = Aabb> {
        self.boxes().map(move |aabb| aabb.offset(x, y, z))
    }

    /// How far `moving` may travel along `axis` before this shape, sitting at the given block, is
    /// in the way.
    #[must_use]
    pub fn collide(self, axis: Axis, moving: &Aabb, at: (f64, f64, f64), mut movement: f64) -> f64 {
        for aabb in self.boxes_at(at.0, at.1, at.2) {
            movement = moving.collide(axis, &aabb, movement);
        }
        movement
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_state::{properties, BlockId, Half};

    fn state(name: &str) -> BlockStateId {
        BlockId::from_name(name)
            .unwrap_or_else(|| panic!("{name} exists"))
            .default_state()
    }

    /// The three the phase set out to get right: a slab is half a block, a fence collides taller
    /// than it draws, and air stops nothing.
    #[test]
    fn blocks_are_the_shape_they_look() {
        let slab = VoxelShape::collision_of(state("minecraft:oak_slab"));
        let boxes: Vec<_> = slab.boxes().collect();
        assert_eq!(boxes.len(), 1);
        assert_eq!(boxes[0].max_y, 0.5, "a bottom slab is half a block tall");

        let fence = VoxelShape::collision_of(state("minecraft:oak_fence"));
        let tallest = fence
            .boxes()
            .map(|aabb| aabb.max_y)
            .fold(f64::MIN, f64::max);
        assert_eq!(tallest, 1.5, "a fence collides half a block above itself");

        assert!(VoxelShape::collision_of(state("minecraft:air")).is_empty());
        assert!(
            VoxelShape::collision_of(state("minecraft:torch")).is_empty(),
            "a torch is walked through"
        );
    }

    /// A closed trapdoor is thin but solid, and standing on one has to stop the fall at its top
    /// rather than at the block below.
    #[test]
    fn a_closed_trapdoor_holds_a_falling_entity() {
        let trapdoor = state("minecraft:oak_trapdoor")
            .with(properties::HALF, Half::Bottom)
            .expect("trapdoors have a half")
            .with(properties::OPEN, false)
            .expect("trapdoors open");
        let shape = VoxelShape::collision_of(trapdoor);

        // A player-sized box falling from just above the block it sits in.
        let falling = Aabb::new(0.2, 1.0, 0.2, 0.8, 2.8, 0.8);
        let stopped = shape.collide(Axis::Y, &falling, (0.0, 0.0, 0.0), -1.0);

        let top = shape
            .boxes()
            .map(|aabb| aabb.max_y)
            .fold(f64::MIN, f64::max);
        assert!(top > 0.0 && top < 1.0, "a closed trapdoor is thin");
        assert_eq!(
            stopped,
            top - 1.0,
            "the fall should stop on the trapdoor, not below it"
        );
    }

    /// Movement away from a block, or past it on another axis, is not shortened.
    #[test]
    fn a_block_only_stops_what_runs_into_it() {
        let stone = VoxelShape::collision_of(state("minecraft:stone"));
        let mover = Aabb::new(0.2, 1.0, 0.2, 0.8, 2.8, 0.8);

        assert_eq!(
            stone.collide(Axis::Y, &mover, (0.0, 0.0, 0.0), 1.0),
            1.0,
            "moving up away from it is unhindered"
        );
        assert_eq!(
            stone.collide(Axis::Y, &mover, (0.0, 0.0, 0.0), -1.0),
            0.0,
            "moving down onto it stops at its top"
        );
        assert_eq!(
            stone.collide(Axis::Y, &mover, (5.0, 0.0, 0.0), -1.0),
            -1.0,
            "a block five along is not in the way"
        );
    }

    /// A slab lets an entity stand half a block higher than the floor beside it.
    #[test]
    fn a_slab_stops_a_fall_halfway() {
        let slab = VoxelShape::collision_of(state("minecraft:oak_slab"));
        let falling = Aabb::new(0.2, 1.0, 0.2, 0.8, 2.8, 0.8);
        assert_eq!(slab.collide(Axis::Y, &falling, (0.0, 0.0, 0.0), -1.0), -0.5);
    }
}
