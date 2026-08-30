//! Facts about a block state that live in the game's code rather than in its data.
//!
//! Extracted by `scripts/extract_block_shapes.py`; see `docs/world/blocks.md`.

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
