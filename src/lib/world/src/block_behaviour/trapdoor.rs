//! Trapdoors, following `TrapDoorBlock` in the vanilla sources.

use super::door::opens_by_hand;
use super::{BlockBehaviour, InteractionResult, Use};
use crate::block_state::properties;
use crate::block_state_id::BlockStateId;
use crate::scheduler::{TickKind, TickPriority};

/// How long water waits before spreading, from `WaterFluid.getTickDelay`.
const WATER_TICK_DELAY: u64 = 5;

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

        // A trapdoor that opens under water lets the water through, so the water is asked to work
        // out where it goes.
        if toggled.get(properties::WATERLOGGED) == Some(true) {
            ctx.world.schedule_tick(
                ctx.pos,
                TickKind::FluidSpread,
                WATER_TICK_DELAY,
                TickPriority::Normal,
            );
        }
        InteractionResult::Success
    }
}
