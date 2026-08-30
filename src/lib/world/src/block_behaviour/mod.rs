//! What a block does, as opposed to what it is.
//!
//! Vanilla dispatches this virtually: 326 block classes each overriding the handful of methods
//! they care about. Most blocks override none, so rather than a trait object per block there is one
//! table keyed on block, holding a behaviour only where there is one to hold. A block with no entry
//! costs a bounds check.
//!
//! Blocks are registered by the vanilla tag that groups them, so adding a wood type adds a door
//! without anything here changing.
//!
//! The method names follow `BlockBehaviour` in the vanilla sources, so the two can be read side by
//! side.

mod door;
mod fence_gate;
mod trapdoor;

use crate::block_state::{BlockId, Direction};
use crate::block_state_id::BlockStateId;
use crate::block_tag::tag;
use crate::pos::BlockPos;
use std::sync::LazyLock;

/// The world, as much of it as a block behaviour may touch.
///
/// Behaviour lives below the ECS and cannot reach into it, so what it is allowed to do is named
/// here and provided from above.
pub trait BlockWorld {
    fn block_at(&mut self, pos: BlockPos) -> BlockStateId;
    fn set_block(&mut self, pos: BlockPos, state: BlockStateId);
}

/// What came of an interaction, which decides whether anything else gets a turn at it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionResult {
    /// Nothing happened here; whatever is holding the item may still act.
    Pass,
    /// The block dealt with it.
    Success,
}

/// A player using a block with an empty hand.
pub struct Use<'a> {
    pub world: &'a mut dyn BlockWorld,
    pub pos: BlockPos,
    /// Which way the player is facing, which a fence gate swings to.
    pub player_facing: Direction,
}

/// What a block does. Every method has an answer that does nothing, so a block implements only
/// what it has.
pub trait BlockBehaviour: Send + Sync {
    /// A player right-clicked it with an empty hand.
    fn use_without_item(&self, _state: BlockStateId, _ctx: &mut Use<'_>) -> InteractionResult {
        InteractionResult::Pass
    }

    /// What this block becomes when a neighbour changes, which is how a door's two halves stay in
    /// step and how a fence connects. Returning the state unchanged means nothing to do.
    fn update_shape(
        &self,
        state: BlockStateId,
        _towards: Direction,
        _neighbour: BlockStateId,
    ) -> BlockStateId {
        state
    }
}

/// One entry per block, almost all of them empty.
static BEHAVIOURS: LazyLock<Vec<Option<&'static dyn BlockBehaviour>>> = LazyLock::new(|| {
    let mut table: Vec<Option<&'static dyn BlockBehaviour>> =
        vec![None; crate::block_state::generated::BLOCKS.len()];

    register(&mut table, "minecraft:doors", &door::Door);
    register(&mut table, "minecraft:trapdoors", &trapdoor::Trapdoor);
    register(&mut table, "minecraft:fence_gates", &fence_gate::FenceGate);

    table
});

fn register(
    table: &mut [Option<&'static dyn BlockBehaviour>],
    name: &str,
    behaviour: &'static dyn BlockBehaviour,
) {
    let Some(group) = tag(name) else {
        // A tag the version does not have means the blocks are not there either.
        return;
    };
    for block in group.blocks() {
        table[usize::from(block.index())] = Some(behaviour);
    }
}

/// What this block does, if anything.
#[must_use]
pub fn behaviour_of(block: BlockId) -> Option<&'static dyn BlockBehaviour> {
    BEHAVIOURS
        .get(usize::from(block.index()))
        .copied()
        .flatten()
}

/// What a state's block does, if anything.
#[must_use]
pub fn behaviour_at(state: BlockStateId) -> Option<&'static dyn BlockBehaviour> {
    behaviour_of(state.block()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_state::{properties, DoubleBlockHalf};
    use std::collections::HashMap;

    /// A patch of world just big enough to place a door in.
    #[derive(Default)]
    struct Blocks(HashMap<(i32, i32, i32), BlockStateId>);

    impl Blocks {
        fn key(pos: BlockPos) -> (i32, i32, i32) {
            (pos.pos.x, pos.pos.y, pos.pos.z)
        }

        fn air() -> BlockStateId {
            BlockId::from_name("minecraft:air")
                .expect("air exists")
                .default_state()
        }
    }

    impl BlockWorld for Blocks {
        fn block_at(&mut self, pos: BlockPos) -> BlockStateId {
            self.0
                .get(&Self::key(pos))
                .copied()
                .unwrap_or_else(Self::air)
        }

        fn set_block(&mut self, pos: BlockPos, state: BlockStateId) {
            self.0.insert(Self::key(pos), state);
        }
    }

    /// Puts both halves of a door at the origin and returns the lower one's position.
    fn place_door(world: &mut Blocks, name: &str) -> BlockPos {
        let block = BlockId::from_name(name).unwrap_or_else(|| panic!("{name} exists"));
        let lower = block
            .default_state()
            .with(properties::DOUBLE_BLOCK_HALF, DoubleBlockHalf::Lower)
            .expect("doors have halves");
        let upper = lower
            .with(properties::DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper)
            .expect("doors have halves");
        let pos = BlockPos::of(0, 64, 0);
        world.set_block(pos, lower);
        world.set_block(pos.relative(Direction::Up), upper);
        pos
    }

    fn use_at(world: &mut Blocks, pos: BlockPos, facing: Direction) -> InteractionResult {
        let state = world.block_at(pos);
        let Some(behaviour) = behaviour_at(state) else {
            return InteractionResult::Pass;
        };
        let mut ctx = Use {
            world,
            pos,
            player_facing: facing,
        };
        behaviour.use_without_item(state, &mut ctx)
    }

    /// Opening a door has to move both halves. Vanilla gets there by the upper half copying the
    /// lower one when it sees it change, which is also how facing and hinge stay in step.
    #[test]
    fn a_door_opens_on_both_halves() {
        let mut world = Blocks::default();
        let pos = place_door(&mut world, "minecraft:oak_door");
        let upper_pos = pos.relative(Direction::Up);

        assert_eq!(
            use_at(&mut world, pos, Direction::North),
            InteractionResult::Success
        );

        assert_eq!(world.block_at(pos).get(properties::OPEN), Some(true));
        assert_eq!(world.block_at(upper_pos).get(properties::OPEN), Some(true));
        assert_eq!(
            world.block_at(upper_pos).get(properties::DOUBLE_BLOCK_HALF),
            Some(DoubleBlockHalf::Upper),
            "the upper half copies everything but its own half"
        );

        // And closes again from the other half.
        assert_eq!(
            use_at(&mut world, upper_pos, Direction::North),
            InteractionResult::Success
        );
        assert_eq!(world.block_at(pos).get(properties::OPEN), Some(false));
        assert_eq!(world.block_at(upper_pos).get(properties::OPEN), Some(false));
    }

    /// An iron door needs redstone. A hand does nothing to it, and the state must not move.
    #[test]
    fn an_iron_door_does_not_open_by_hand() {
        let mut world = Blocks::default();
        let pos = place_door(&mut world, "minecraft:iron_door");
        let before = world.block_at(pos);

        assert_eq!(
            use_at(&mut world, pos, Direction::North),
            InteractionResult::Pass
        );
        assert_eq!(world.block_at(pos), before);
    }

    /// A gate opened from behind swings away from the player rather than into them.
    #[test]
    fn a_fence_gate_swings_towards_the_player() {
        let mut world = Blocks::default();
        let gate = BlockId::from_name("minecraft:oak_fence_gate").expect("gates exist");
        let pos = BlockPos::of(0, 64, 0);
        let facing_north = gate
            .default_state()
            .with(properties::FACING, Direction::North)
            .expect("gates face");
        world.set_block(pos, facing_north);

        // The player stands on the north side, looking south: the gate is facing away from them.
        use_at(&mut world, pos, Direction::South);
        let opened = world.block_at(pos);
        assert_eq!(opened.get(properties::OPEN), Some(true));
        assert_eq!(
            opened.get(properties::FACING),
            Some(Direction::South),
            "the gate turns to face the player before opening"
        );

        // Closing it again leaves the facing alone.
        use_at(&mut world, pos, Direction::North);
        let closed = world.block_at(pos);
        assert_eq!(closed.get(properties::OPEN), Some(false));
        assert_eq!(closed.get(properties::FACING), Some(Direction::South));
    }

    /// A trapdoor is one block and toggles on its own.
    #[test]
    fn a_trapdoor_toggles() {
        let mut world = Blocks::default();
        let trapdoor = BlockId::from_name("minecraft:oak_trapdoor").expect("trapdoors exist");
        let pos = BlockPos::of(0, 64, 0);
        world.set_block(pos, trapdoor.default_state());

        assert_eq!(
            use_at(&mut world, pos, Direction::North),
            InteractionResult::Success
        );
        assert_eq!(world.block_at(pos).get(properties::OPEN), Some(true));
    }

    /// The point of the table: a block with nothing to do has no entry, and one that does has the
    /// same entry as the rest of its family.
    #[test]
    fn only_blocks_with_behaviour_are_in_the_table() {
        let stone = BlockId::from_name("minecraft:stone").expect("stone exists");
        assert!(behaviour_of(stone).is_none());

        for name in [
            "minecraft:oak_door",
            "minecraft:iron_door",
            "minecraft:copper_door",
            "minecraft:oak_trapdoor",
            "minecraft:oak_fence_gate",
        ] {
            let block = BlockId::from_name(name).unwrap_or_else(|| panic!("{name} exists"));
            assert!(
                behaviour_of(block).is_some(),
                "{name} should have behaviour"
            );
        }
    }
}
