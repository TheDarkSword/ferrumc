//! Doors, following `DoorBlock` in the vanilla sources.

use super::{BlockBehaviour, InteractionResult, Use};
use crate::block_state::{properties, Direction, DoubleBlockHalf};
use crate::block_state_id::BlockStateId;

/// Doors of `BlockSetType.IRON`, which is the one set that a hand cannot work. Everything else,
/// copper included, opens by hand.
const NO_HAND: [&str; 2] = ["minecraft:iron_door", "minecraft:iron_trapdoor"];

pub(super) fn opens_by_hand(state: BlockStateId) -> bool {
    state
        .block()
        .is_none_or(|block| !NO_HAND.contains(&block.name()))
}

pub(super) struct Door;

impl BlockBehaviour for Door {
    fn use_without_item(&self, state: BlockStateId, ctx: &mut Use<'_>) -> InteractionResult {
        if !opens_by_hand(state) {
            return InteractionResult::Pass;
        }
        let Some(open) = state.get(properties::OPEN) else {
            return InteractionResult::Pass;
        };
        let Some(opened) = state.with(properties::OPEN, !open) else {
            return InteractionResult::Pass;
        };
        ctx.world.set_block(ctx.pos, opened);

        // Vanilla leaves this to the neighbour update that follows the change: the other half sees
        // its neighbour and copies it. Neighbour updates are not driven yet, so the same rule is
        // applied here directly, through the same method that will run it later.
        let towards = match state.get(properties::DOUBLE_BLOCK_HALF) {
            Some(DoubleBlockHalf::Lower) => Direction::Up,
            Some(DoubleBlockHalf::Upper) => Direction::Down,
            None => return InteractionResult::Success,
        };
        let other_pos = ctx.pos.relative(towards);
        let other = ctx.world.block_at(other_pos);
        let updated = self.update_shape(other, ctx.world, other_pos, towards.opposite(), opened);
        if updated != other {
            ctx.world.set_block(other_pos, updated);
        }

        InteractionResult::Success
    }

    fn update_shape(
        &self,
        state: BlockStateId,
        _world: &mut dyn super::BlockWorld,
        _pos: crate::pos::BlockPos,
        towards: Direction,
        neighbour: BlockStateId,
    ) -> BlockStateId {
        let Some(half) = state.get(properties::DOUBLE_BLOCK_HALF) else {
            return state;
        };
        // Only the neighbour that is the other half of this door has anything to say.
        let other_half_lies = match half {
            DoubleBlockHalf::Lower => Direction::Up,
            DoubleBlockHalf::Upper => Direction::Down,
        };
        if towards != other_half_lies {
            return state;
        }

        // The half takes on everything its other half holds - open, facing, hinge, powered - and
        // keeps only its own half. Setting `open` on both instead would leave the rest to drift.
        match neighbour.get(properties::DOUBLE_BLOCK_HALF) {
            Some(other) if other != half && neighbour.block() == state.block() => neighbour
                .with(properties::DOUBLE_BLOCK_HALF, half)
                .unwrap_or(state),
            // Nothing above or below to be half of: the door is gone.
            _ => crate::block_state::BlockId::from_name("minecraft:air")
                .map_or(state, crate::block_state::BlockId::default_state),
        }
    }
}
