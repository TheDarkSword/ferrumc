//! Sugar cane, following `SugarCaneBlock` in the vanilla sources.

use super::{BlockBehaviour, Tick};
use crate::block_state::properties;
use crate::block_state::{BlockId, Direction};
use crate::block_state_id::BlockStateId;
use crate::pos::BlockPos;

/// A stalk grows to three blocks and stops.
const MAX_HEIGHT: i32 = 3;
/// It counts to fifteen before putting on a block, so a stalk is fifteen random ticks apart.
const RIPE: u8 = 15;

pub(super) struct SugarCane;

impl BlockBehaviour for SugarCane {
    fn random_tick(&self, state: BlockStateId, ctx: &mut Tick<'_>) {
        // Nothing happens under anything, even another cane.
        let above = ctx.pos.relative(Direction::Up);
        if !is_air(ctx.world.block_at(above)) {
            return;
        }

        // How tall the stalk already is, counted downwards from here.
        let mut height = 1;
        while height < MAX_HEIGHT {
            let below = BlockPos::of(ctx.pos.pos.x, ctx.pos.pos.y - height, ctx.pos.pos.z);
            if ctx.world.block_at(below).block() != state.block() {
                break;
            }
            height += 1;
        }
        if height >= MAX_HEIGHT {
            return;
        }

        let Some(age) = state.get(properties::AGE) else {
            return;
        };
        if age == RIPE {
            let Some(block) = state.block() else {
                return;
            };
            ctx.world.set_block(above, block.default_state());
            if let Some(reset) = state.with(properties::AGE, 0) {
                ctx.world.set_block(ctx.pos, reset);
            }
        } else if let Some(older) = state.with(properties::AGE, age + 1) {
            ctx.world.set_block(ctx.pos, older);
        }
    }
}

fn is_air(state: BlockStateId) -> bool {
    state
        .block()
        .and_then(|block| BlockId::from_name("minecraft:air").map(|air| block == air))
        .unwrap_or(false)
}
