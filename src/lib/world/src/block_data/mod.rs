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

/// Two bytes per state: what it emits and how much it dims light, then the flags the engines
/// branch on.
static LIGHT: &[u8] = include_bytes!("../../../../../assets/data/block_shapes/light.bin");

/// The brightest light there is.
pub const MAX_LIGHT: u8 = 15;

fn light_bytes(state: BlockStateId) -> (u8, u8) {
    let index = state.raw() as usize * 2;
    match LIGHT.get(index..index + 2) {
        Some([value, flags]) => (*value, *flags),
        // An id this server does not generate. Treating it as solid darkness is safer than as air.
        _ => (MAX_LIGHT << 4, 0),
    }
}

/// How much light this state gives off, zero to fifteen.
#[must_use]
pub fn light_emission(state: BlockStateId) -> u8 {
    light_bytes(state).0 & 0x0F
}

/// How much this state dims light, as the game states it.
///
/// Zero means it does not dim light at all, which is also what says skylight carries straight down
/// through it. [`light_opacity`] is this raised to at least one, which is what the spreading needs.
#[must_use]
pub fn light_dampening(state: BlockStateId) -> u8 {
    light_bytes(state).0 >> 4
}

/// How much a light level drops crossing this state.
///
/// Never less than one, because light has to run out even in air: the engine subtracts this per
/// block, and a zero would let it travel for ever.
#[must_use]
pub fn light_opacity(state: BlockStateId) -> u8 {
    (light_bytes(state).0 >> 4).max(1)
}

/// Whether skylight carries straight down through this state without dimming.
#[must_use]
pub fn propagates_skylight(state: BlockStateId) -> bool {
    light_bytes(state).1 & 2 != 0
}

/// Which face shape each of a state's six sides has.
static FACE_SHAPES: &[u8] =
    include_bytes!("../../../../../assets/data/block_shapes/face_shapes.bin");

/// Whether light is stopped between a pair of face shapes, one bit each way round.
static FACE_OCCLUSION: &[u8] =
    include_bytes!("../../../../../assets/data/block_shapes/face_occlusion.bin");

/// How many distinct face shapes there are, which is the matrix's side.
const FACE_SHAPE_COUNT: usize = 55;
const FACE_ROW: usize = FACE_SHAPE_COUNT.div_ceil(8);

const fn face_index(face: Direction) -> usize {
    match face {
        Direction::Down => 0,
        Direction::Up => 1,
        Direction::North => 2,
        Direction::South => 3,
        Direction::West => 4,
        Direction::East => 5,
    }
}

fn face_shape(state: BlockStateId, face: Direction) -> usize {
    FACE_SHAPES
        .get(state.raw() as usize * 6 + face_index(face))
        .copied()
        .unwrap_or(0) as usize
}

/// Whether light is stopped between two blocks, `from` looking `towards` `to`.
///
/// A slab dims nothing — its opacity is zero — and still stops light through its flat side, so this
/// is not a question about opacity. Nor is it a question about either face alone: two partial faces
/// can cover the opening between them while neither covers it by itself. There are only 55 distinct
/// faces, so every pair's answer is worked out once and looked up here.
#[must_use]
pub fn shape_occludes_between(from: BlockStateId, to: BlockStateId, towards: Direction) -> bool {
    let a = face_shape(from, towards);
    let b = face_shape(to, towards.opposite());
    FACE_OCCLUSION
        .get(a * FACE_ROW + b / 8)
        .is_some_and(|byte| byte & (1 << (b % 8)) != 0)
}

/// Which block entity each block carries, as a registry id.
static BLOCK_ENTITIES: &[u8] =
    include_bytes!("../../../../../assets/data/block_shapes/block_entities.bin");

/// Written where a block carries none.
const NO_BLOCK_ENTITY: u16 = u16::MAX;

/// The block entity this block carries, if it carries one.
///
/// Keyed on the block rather than the state: every state of a chest is a chest. 186 of 1196 blocks
/// carry one.
#[must_use]
pub fn block_entity_type(block: crate::block_state::BlockId) -> Option<u16> {
    let index = usize::from(block.index()) * 2;
    let bytes = BLOCK_ENTITIES.get(index..index + 2)?;
    let id = u16::from_le_bytes([bytes[0], bytes[1]]);
    (id != NO_BLOCK_ENTITY).then_some(id)
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

    /// What each block does to light. A torch gives it off, stone stops it, glass lets it by, and
    /// leaves and water take one off it.
    #[test]
    fn blocks_deal_with_light_differently() {
        let state = |name: &str| {
            BlockId::from_name(name)
                .unwrap_or_else(|| panic!("{name} exists"))
                .default_state()
        };

        assert_eq!(light_emission(state("minecraft:torch")), 14);
        assert_eq!(light_emission(state("minecraft:stone")), 0);

        assert_eq!(light_opacity(state("minecraft:stone")), 15);
        assert_eq!(light_opacity(state("minecraft:water")), 1);
        // Air dims light by one even though it stops nothing, or light would never run out.
        assert_eq!(light_opacity(state("minecraft:air")), 1);
        assert_eq!(light_opacity(state("minecraft:glass")), 1);

        assert!(propagates_skylight(state("minecraft:air")));
        assert!(propagates_skylight(state("minecraft:glass")));
        assert!(!propagates_skylight(state("minecraft:stone")));
    }

    /// A slab dims nothing and still stops light through its flat side, so this is not a question
    /// about opacity. Which way matters: a bottom slab's underside is full and its top is not
    /// there at all.
    #[test]
    fn a_face_can_stop_light_on_its_own() {
        let state = |name: &str| {
            BlockId::from_name(name)
                .unwrap_or_else(|| panic!("{name} exists"))
                .default_state()
        };
        let slab = state("minecraft:oak_slab");
        let air = state("minecraft:air");
        assert_eq!(light_opacity(slab), 1);

        // Down out of the slab, its full underside is in the way.
        assert!(shape_occludes_between(slab, air, Direction::Down));
        // Down into it from above, nothing is: its top is half a block below the face.
        assert!(!shape_occludes_between(air, slab, Direction::Down));

        // Stone stops light by being opaque, so nothing about its faces stops anything.
        assert!(!shape_occludes_between(
            state("minecraft:stone"),
            air,
            Direction::Down
        ));
    }

    /// The case a table of single faces cannot answer, and the reason every pair is worked out
    /// instead: a top slab beside a bottom slab closes the way between them, while neither side
    /// closes it alone.
    #[test]
    fn two_partial_faces_stop_light_together() {
        use crate::block_state::{properties, SlabType};

        let slab = BlockId::from_name("minecraft:oak_slab").expect("slabs exist");
        let bottom = slab
            .default_state()
            .with(properties::SLAB_TYPE, SlabType::Bottom)
            .expect("slabs have a type");
        let top = slab
            .default_state()
            .with(properties::SLAB_TYPE, SlabType::Top)
            .expect("slabs have a type");
        let air = BlockId::from_name("minecraft:air")
            .expect("air exists")
            .default_state();

        assert!(
            !shape_occludes_between(top, air, Direction::North),
            "a top slab's side does not close the way on its own"
        );
        assert!(
            !shape_occludes_between(air, bottom, Direction::North),
            "nor does a bottom slab's"
        );
        assert!(
            shape_occludes_between(top, bottom, Direction::North),
            "together they cover the whole opening"
        );
    }

    /// A chest holds more than its state id can say; stone does not.
    #[test]
    fn some_blocks_carry_a_block_entity() {
        let block =
            |name: &str| BlockId::from_name(name).unwrap_or_else(|| panic!("{name} exists"));

        assert!(block_entity_type(block("minecraft:chest")).is_some());
        assert!(block_entity_type(block("minecraft:oak_sign")).is_some());
        assert!(block_entity_type(block("minecraft:furnace")).is_some());
        assert!(block_entity_type(block("minecraft:stone")).is_none());
        assert!(block_entity_type(block("minecraft:air")).is_none());

        // Every sign is the same kind of block entity, wall ones included.
        assert_eq!(
            block_entity_type(block("minecraft:oak_sign")),
            block_entity_type(block("minecraft:oak_wall_sign")),
        );
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
