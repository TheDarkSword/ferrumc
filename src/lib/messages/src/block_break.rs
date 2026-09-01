use bevy_ecs::prelude::Message;
use ferrumc_world::pos::BlockPos;

/// Message sent when a block is broken in the world
#[derive(Message)]
pub struct BlockBrokenEvent {
    pub position: BlockPos,
    /// What was there. By the time anyone reads this the position holds air, and what a block
    /// leaves behind depends on which block it was.
    pub state: ferrumc_world::block_state_id::BlockStateId,
}
