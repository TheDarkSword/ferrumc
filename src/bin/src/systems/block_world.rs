//! The world as a block behaviour sees it.
//!
//! Behaviour lives below the ECS and reaches the world through this. Each read and write takes the
//! chunk guard and gives it back before the next one: holding two guards on one shard deadlocks the
//! tick thread.

use ferrumc_config::server_config::get_global_config;
use ferrumc_core::transform::position::Position;
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::block_update::BlockUpdate;
use ferrumc_net_codec::net_types::network_position::NetworkPosition;
use ferrumc_state::GlobalState;
use ferrumc_world::block_behaviour::BlockWorld;
use ferrumc_world::block_state_id::BlockStateId;
use ferrumc_world::chunk::remap::NetworkBlockState;
use ferrumc_world::pos::BlockPos;
use ferrumc_world::scheduler::{BlockTickScheduler, TickKind, TickPriority};
use tracing::error;

pub struct WorldAccess<'a> {
    state: &'a GlobalState,
    scheduler: &'a mut BlockTickScheduler,
    current_tick: u64,
    /// Every block a behaviour changed, so the players who can see them can be told.
    pub changed: Vec<(BlockPos, BlockStateId)>,
}

impl<'a> WorldAccess<'a> {
    pub fn new(
        state: &'a GlobalState,
        scheduler: &'a mut BlockTickScheduler,
        current_tick: u64,
    ) -> Self {
        Self {
            state,
            scheduler,
            current_tick,
            changed: Vec::new(),
        }
    }
}

impl BlockWorld for WorldAccess<'_> {
    fn block_at(&mut self, pos: BlockPos) -> BlockStateId {
        match ferrumc_utils::world::load_or_generate_mut(self.state, pos.chunk(), "overworld") {
            Ok(chunk) => chunk.get_block(pos.chunk_block_pos()),
            Err(err) => {
                error!("Could not read the block at {}: {:?}", pos, err);
                BlockStateId::new(0)
            }
        }
    }

    fn set_block(&mut self, pos: BlockPos, state: BlockStateId) {
        match ferrumc_utils::world::load_or_generate_mut(self.state, pos.chunk(), "overworld") {
            Ok(mut chunk) => {
                chunk.set_block(pos.chunk_block_pos(), state);
                self.changed.push((pos, state));
            }
            Err(err) => error!("Could not set the block at {}: {:?}", pos, err),
        }
    }

    fn schedule_tick(&mut self, pos: BlockPos, kind: TickKind, delay: u64, priority: TickPriority) {
        self.scheduler
            .schedule_with_priority(pos, kind, self.current_tick, delay, priority);
    }
}

/// Tells everyone in range about blocks a behaviour changed.
pub fn broadcast_changes<'a>(
    changed: &[(BlockPos, BlockStateId)],
    players: impl Iterator<Item = (&'a StreamWriter, &'a Position)>,
) {
    if changed.is_empty() {
        return;
    }
    let render_distance = get_global_config().chunk_render_distance as i32;
    let players: Vec<_> = players.collect();
    for &(pos, state) in changed {
        let packet = BlockUpdate {
            location: NetworkPosition {
                x: pos.pos.x,
                y: pos.pos.y as i16,
                z: pos.pos.z,
            },
            block_state_id: NetworkBlockState::from(state),
        };
        let chunk = pos.chunk();
        for (conn, player) in &players {
            let player_chunk = player.chunk();
            if (chunk.x() - player_chunk.x).abs() <= render_distance
                && (chunk.z() - player_chunk.y).abs() <= render_distance
            {
                if let Err(err) = conn.send_packet_ref(&packet) {
                    error!("Failed to send block update packet: {:?}", err);
                }
            }
        }
    }
}
