//! Standing torches, following `BaseTorchBlock` in the vanilla sources.
//!
//! A wall torch is a different block with a different rule and is not this.

use super::{BlockBehaviour, BlockWorld};
use crate::block_data::{face_sturdy, SupportType};
use crate::block_state::{BlockId, Direction};
use crate::block_state_id::BlockStateId;
use crate::pos::BlockPos;

pub(super) struct Torch;

impl BlockBehaviour for Torch {
    fn can_survive(&self, _state: BlockStateId, world: &mut dyn BlockWorld, pos: BlockPos) -> bool {
        // Only the centre of the face has to hold it, which is why a torch sits on a fence post.
        let below = world.block_at(pos.relative(Direction::Down));
        face_sturdy(below, Direction::Up, SupportType::Centre)
    }

    fn update_shape(
        &self,
        state: BlockStateId,
        world: &mut dyn BlockWorld,
        pos: BlockPos,
        towards: Direction,
        _neighbour: BlockStateId,
    ) -> BlockStateId {
        // Only what changed underneath can take its support away.
        if towards == Direction::Down && !self.can_survive(state, world, pos) {
            return BlockId::from_name("minecraft:air").map_or(state, BlockId::default_state);
        }
        state
    }
}
