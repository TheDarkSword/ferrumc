//! Giving blocks their turns.
//!
//! Two independent systems, as in vanilla:
//!
//! - **Scheduled ticks** are the ones a block asked for. Their order within a game tick is
//!   observable — it is why redstone reads as it does — so they run by priority and then by which
//!   was asked for first, across the whole world rather than per chunk.
//! - **Random ticks** are handed out: every section that holds anything worth ticking gets
//!   `random_tick_speed` positions a tick, chosen the way vanilla chooses them.

use bevy_ecs::prelude::{Query, Res, ResMut, Resource};
use ferrumc_core::tick::TickCounter;
use ferrumc_core::transform::position::Position;
use ferrumc_net::connection::StreamWriter;
use ferrumc_state::GlobalStateResource;
use ferrumc_world::block_behaviour::{behaviour_at, BlockWorld, Tick};
use ferrumc_world::block_data::randomly_ticking;
use ferrumc_world::block_state_id::BlockStateId;
use ferrumc_world::pos::{BlockPos, ChunkPos};
use ferrumc_world::scheduler::TickKind;

use super::block_world::WorldAccess;
use super::fluids::FluidScheduler;

/// How many positions each section is handed a tick. Vanilla's `random_tick_speed` game rule,
/// which defaults to three.
const RANDOM_TICK_SPEED: usize = 3;

/// How many scheduled ticks one game tick will run before leaving the rest for the next.
const MAX_SCHEDULED_TICKS: usize = 65_536;

/// The counter vanilla picks random tick positions from.
///
/// Not a general random source: it is one multiply and add per position, shared by every section,
/// which is what keeps handing out thousands of positions a second cheap.
#[derive(Resource, Default)]
pub struct RandomTickPositions {
    value: i32,
}

impl RandomTickPositions {
    /// A position inside the section whose corner is given, following `Level.getBlockRandomPos`.
    fn next(&mut self, corner_x: i32, corner_y: i32, corner_z: i32) -> BlockPos {
        self.value = self.value.wrapping_mul(3).wrapping_add(1_013_904_223);
        let value = self.value >> 2;
        BlockPos::of(
            corner_x + (value & 15),
            corner_y + ((value >> 16) & 15),
            corner_z + ((value >> 8) & 15),
        )
    }
}

/// Runs the ticks blocks asked for, in the order they have to run in.
pub fn scheduled(
    state: Res<GlobalStateResource>,
    mut scheduler: ResMut<FluidScheduler>,
    tick: Res<TickCounter>,
    player_query: Query<(&StreamWriter, &Position)>,
    levels: Res<super::chunk_levels::Levels>,
) {
    let current = tick.get();
    let due = scheduler
        .0
        .drain_ordered(current, MAX_SCHEDULED_TICKS, TickKind::Block);
    if due.is_empty() {
        return;
    }
    // A tick due in a chunk that is not simulated waits rather than running: vanilla leaves it in
    // the chunk, and putting it back is how it stays due without being lost.
    let (due, waiting): (Vec<_>, Vec<_>) = due
        .into_iter()
        .partition(|tick| levels.0.status(tick.pos.chunk()).ticks_blocks());
    for tick in waiting {
        scheduler
            .0
            .schedule_with_priority(tick.pos, tick.kind, current, 0, tick.priority);
    }
    if due.is_empty() {
        return;
    }

    let mut world = WorldAccess::new(&state.0, &mut scheduler.0, current);
    for scheduled in due {
        let block = world.block_at(scheduled.pos);
        if let Some(behaviour) = behaviour_at(block) {
            let mut ctx = Tick {
                world: &mut world,
                pos: scheduled.pos,
            };
            behaviour.scheduled_tick(block, &mut ctx);
        }
    }

    let changed = std::mem::take(&mut world.changed);
    super::block_world::broadcast_changes(&changed, player_query.iter());
}

/// Hands out random ticks to the sections that have anything to do with them.
pub fn random(
    state: Res<GlobalStateResource>,
    mut scheduler: ResMut<FluidScheduler>,
    tick: Res<TickCounter>,
    mut positions: ResMut<RandomTickPositions>,
    player_query: Query<(&StreamWriter, &Position)>,
    levels: Res<super::chunk_levels::Levels>,
) {
    let players: Vec<_> = player_query.iter().collect();
    // The chunks are listed first and the map released, so loading a chunk to write to it later
    // cannot deadlock against the iterator.
    // Only what is close enough to a player to be simulated. A chunk at the edge of what someone
    // can see is kept and sent; nothing in it grows.
    let chunks: Vec<ChunkPos> = state
        .0
        .world
        .get_cache()
        .iter()
        .filter(|entry| entry.key().1 == "overworld")
        .map(|entry| entry.key().0)
        .filter(|&pos| levels.0.status(pos).ticks_blocks())
        .collect();

    let current = tick.get();
    let mut candidates: Vec<(BlockPos, BlockStateId)> = Vec::new();
    for chunk_pos in chunks {
        let Ok(chunk) =
            ferrumc_utils::world::load_or_generate_mut(&state.0, chunk_pos, "overworld")
        else {
            continue;
        };
        let corner_x = chunk_pos.x() * 16;
        let corner_z = chunk_pos.z() * 16;
        for (index, section) in chunk.sections.iter().enumerate() {
            // A section with nothing that ticks is skipped rather than sampled.
            if !section.any_block(randomly_ticking) {
                continue;
            }
            let corner_y = i32::from(chunk.dimensions().min_y) + (index as i32) * 16;
            for _ in 0..RANDOM_TICK_SPEED {
                let pos = positions.next(corner_x, corner_y, corner_z);
                let block = chunk.get_block(pos.chunk_block_pos());
                if randomly_ticking(block) {
                    candidates.push((pos, block));
                }
            }
        }
    }

    if candidates.is_empty() {
        return;
    }

    let mut world = WorldAccess::new(&state.0, &mut scheduler.0, current);
    for (pos, block) in candidates {
        if let Some(behaviour) = behaviour_at(block) {
            let mut ctx = Tick {
                world: &mut world,
                pos,
            };
            behaviour.random_tick(block, &mut ctx);
        }
    }
    let changed = std::mem::take(&mut world.changed);
    super::block_world::broadcast_changes(&changed, players.iter().copied());
}

/// Hands a newly loaded chunk's waiting turns to the scheduler.
///
/// A loaded chunk's ticks belong in the scheduler, where they can be ordered against every other
/// chunk's. Going the other way happens where the chunk is actually let go of, in the unloader,
/// rather than here: vanilla registers and unregisters a chunk's tick container as it is loaded and
/// unloaded, and doing it at those two points is what stops the two ever disagreeing.
pub fn carry_ticks(
    state: Res<GlobalStateResource>,
    mut scheduler: ResMut<FluidScheduler>,
    tick: Res<TickCounter>,
    levels: Res<super::chunk_levels::Levels>,
) {
    let current = tick.get();

    // Chunks that have just been loaded hand over what they were holding.
    let waiting: Vec<ChunkPos> = state
        .0
        .world
        .get_cache()
        .iter()
        .filter(|entry| entry.key().1 == "overworld" && !entry.value().scheduled_ticks().is_empty())
        .map(|entry| entry.key().0)
        .filter(|&pos| levels.0.status(pos).is_loaded())
        .collect();
    for pos in waiting {
        let Ok(mut chunk) = ferrumc_utils::world::load_or_generate_mut(&state.0, pos, "overworld")
        else {
            continue;
        };
        let held = chunk.take_scheduled_ticks();
        drop(chunk);
        scheduler.0.restore_chunk(&held, current);
    }
}
