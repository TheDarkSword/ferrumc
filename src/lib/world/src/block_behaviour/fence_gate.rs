//! Fence gates, following `FenceGateBlock` in the vanilla sources.

use super::{BlockBehaviour, InteractionResult, Use};
use crate::block_state::properties;
use crate::block_state_id::BlockStateId;

pub(super) struct FenceGate;

impl BlockBehaviour for FenceGate {
    fn use_without_item(&self, state: BlockStateId, ctx: &mut Use<'_>) -> InteractionResult {
        let Some(open) = state.get(properties::OPEN) else {
            return InteractionResult::Pass;
        };

        let swung = if open {
            state.with(properties::OPEN, false)
        } else {
            // A gate opened from its far side swings towards the player rather than into them.
            let facing = state.get(properties::FACING);
            let turned = match facing {
                Some(facing) if facing == ctx.player_facing.opposite() => {
                    state.with(properties::FACING, ctx.player_facing)
                }
                _ => Some(state),
            };
            turned.and_then(|state| state.with(properties::OPEN, true))
        };

        let Some(swung) = swung else {
            return InteractionResult::Pass;
        };
        ctx.world.set_block(ctx.pos, swung);
        InteractionResult::Success
    }
}
