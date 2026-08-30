//! Trapdoors, following `TrapDoorBlock` in the vanilla sources.

use super::door::opens_by_hand;
use super::{BlockBehaviour, InteractionResult, Use};
use crate::block_state::properties;
use crate::block_state_id::BlockStateId;

pub(super) struct Trapdoor;

impl BlockBehaviour for Trapdoor {
    fn use_without_item(&self, state: BlockStateId, ctx: &mut Use<'_>) -> InteractionResult {
        if !opens_by_hand(state) {
            return InteractionResult::Pass;
        }
        let Some(open) = state.get(properties::OPEN) else {
            return InteractionResult::Pass;
        };
        let Some(toggled) = state.with(properties::OPEN, !open) else {
            return InteractionResult::Pass;
        };
        ctx.world.set_block(ctx.pos, toggled);
        // A waterlogged trapdoor also asks the water to tick; scheduled ticks are not built yet.
        InteractionResult::Success
    }
}
