//! Facts about a block state that live in the game's code rather than in its data.
//!
//! Extracted by `scripts/extract_block_shapes.py`; see `docs/world/blocks.md`.

use crate::block_state::Direction;
use crate::block_state_id::BlockStateId;

/// One bit per state, saying whether it takes a random tick.
static RANDOMLY_TICKING: &[u8] =
    include_bytes!("../../../../../assets/data/block_shapes/randomly_ticking.bin");

/// Whether the world gives this state a turn at random.
///
/// 1508 of 32366 states do. The random tick loop asks this of thousands of positions a second, so
/// it is one bit test.
#[must_use]
pub fn randomly_ticking(state: BlockStateId) -> bool {
    let index = state.raw() as usize;
    RANDOMLY_TICKING
        .get(index / 8)
        .is_some_and(|byte| byte & (1 << (index % 8)) != 0)
}

/// Three bytes per state: one bit per face and support type.
static FACE_STURDY: &[u8] =
    include_bytes!("../../../../../assets/data/block_shapes/face_sturdy.bin");

/// How much of a face something needs to rest on it.
///
/// Vanilla's `SupportType`, in its own order: a full face, only its centre, or a rigid centre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportType {
    Full,
    Centre,
    Rigid,
}

/// Whether a face of this block will hold something up.
///
/// This is what a torch asks before staying where it is and what a door asks before standing on
/// something. It is an answer about the block's support shape rather than the shape itself, which
/// is what every caller wants and saves carrying a fourth shape per state.
#[must_use]
pub fn face_sturdy(state: BlockStateId, face: Direction, support: SupportType) -> bool {
    let index = state.raw() as usize * 3;
    let Some(bytes) = FACE_STURDY.get(index..index + 3) else {
        return false;
    };
    let bits = u32::from(bytes[0]) | u32::from(bytes[1]) << 8 | u32::from(bytes[2]) << 16;
    let direction = match face {
        Direction::Down => 0,
        Direction::Up => 1,
        Direction::North => 2,
        Direction::South => 3,
        Direction::West => 4,
        Direction::East => 5,
    };
    let support = match support {
        SupportType::Full => 0,
        SupportType::Centre => 1,
        SupportType::Rigid => 2,
    };
    bits & (1 << (direction * 3 + support)) != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_state::{properties, BlockId};

    /// Crops and saplings grow on their own; stone does not.
    #[test]
    fn only_some_blocks_take_a_random_tick() {
        let ticking = |name: &str| {
            randomly_ticking(
                BlockId::from_name(name)
                    .unwrap_or_else(|| panic!("{name} exists"))
                    .default_state(),
            )
        };

        assert!(ticking("minecraft:wheat"));
        assert!(ticking("minecraft:sugar_cane"));
        assert!(ticking("minecraft:oak_sapling"));
        assert!(!ticking("minecraft:stone"));
        assert!(!ticking("minecraft:air"));
    }

    /// What a torch can stand on. A fence holds one up by its centre without being a full block;
    /// a bottom slab holds nothing, since its top is half a block below the face.
    #[test]
    fn a_face_holds_things_up_or_does_not() {
        let state = |name: &str| {
            BlockId::from_name(name)
                .unwrap_or_else(|| panic!("{name} exists"))
                .default_state()
        };

        assert!(face_sturdy(
            state("minecraft:stone"),
            Direction::Up,
            SupportType::Full
        ));
        assert!(face_sturdy(
            state("minecraft:stone"),
            Direction::Up,
            SupportType::Centre
        ));

        // A fence is not a full block but a torch still sits on it.
        assert!(!face_sturdy(
            state("minecraft:oak_fence"),
            Direction::Up,
            SupportType::Full
        ));
        assert!(face_sturdy(
            state("minecraft:oak_fence"),
            Direction::Up,
            SupportType::Centre
        ));

        // A bottom slab's top is half a block down, so its up face holds nothing.
        assert!(!face_sturdy(
            state("minecraft:oak_slab"),
            Direction::Up,
            SupportType::Centre
        ));
        assert!(!face_sturdy(
            state("minecraft:air"),
            Direction::Up,
            SupportType::Centre
        ));
    }

    /// It is a property of the state, not of the block: a fully grown crop is done growing.
    #[test]
    fn a_grown_crop_stops_ticking() {
        let wheat = BlockId::from_name("minecraft:wheat").expect("wheat exists");
        let grown = wheat
            .default_state()
            .with(properties::AGE, 7)
            .expect("wheat ages");
        assert!(randomly_ticking(wheat.default_state()));
        assert!(!randomly_ticking(grown));
    }
}
