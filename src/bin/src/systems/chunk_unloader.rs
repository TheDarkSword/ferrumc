use bevy_ecs::prelude::{Query, Res, ResMut};
use ferrumc_core::chunks::chunk_receiver::ChunkReceiver;
use ferrumc_state::GlobalStateResource;
use ferrumc_world::pos::ChunkPos;
use std::collections::HashSet;
use tracing::{error, trace};

/// Lets go of the chunks nothing is holding on to.
///
/// What is held is the levels' answer, not a second one of this system's own. Two answers to that
/// question is how a chunk ends up with its scheduled ticks taken away for leaving and then never
/// leaving: the ticks are handed back and taken again every tick, and never run.
pub fn handle(
    state: Res<GlobalStateResource>,
    query: Query<&ChunkReceiver>,
    levels: Res<crate::systems::chunk_levels::Levels>,
    mut scheduler: ResMut<crate::systems::fluids::FluidScheduler>,
    tick: Res<ferrumc_core::tick::TickCounter>,
) {
    // If there are no connected players, unload all cached chunks
    if query.count() == 0 {
        let mut removed = 0;
        for chunk_candidate in state.0.world.get_cache() {
            let ((pos, dim), chunk) = chunk_candidate.pair();
            removed += 1;
            // Write chunks back to the world storage
            if chunk.sections.iter().any(|section| section.dirty) {
                state
                    .0
                    .world
                    .insert_chunk(*pos, dim.as_str(), chunk.clone())
                    .expect("Failed to re-insert chunk after unloading from cache.");
                continue;
            }
        }
        // Clear the entire cache
        state.0.world.get_cache().clear();
        // Log how many chunks were removed
        if removed > 0 {
            trace!(
                "Unloaded {} chunks from cache as there are no connected players.",
                removed
            );
        }
        return;
    }
    let mut all_chunks: HashSet<ChunkPos> = HashSet::new();
    let mut visible_chunks = HashSet::new();
    for chunk_candidate in state.0.world.get_cache() {
        let (key, _) = chunk_candidate.pair();
        all_chunks.insert(key.0);
        // Whether anything is holding on to it is the levels' answer.
        if levels.0.status(key.0).is_loaded() {
            visible_chunks.insert(key.0);
        }
    }
    let mut unloaded_entries = 0;
    let mut written_chunks = 0;
    // The difference is the set of chunks that are in the cache but not visible to any player
    for chunk_pos in all_chunks.difference(&visible_chunks) {
        let removed_chunk = state
            .0
            .world
            .get_cache()
            .remove(&(*chunk_pos, "overworld".to_string()));
        // Whatever it was waiting to do goes with it, so it is written out and still due when the
        // chunk comes back.
        let held = scheduler.0.take_chunk(*chunk_pos, tick.get());

        match removed_chunk {
            Some(((pos, dim), mut chunk)) => {
                if !held.is_empty() {
                    chunk.hold_scheduled_ticks(held);
                }
                let dirty = chunk.sections.iter().any(|section| section.dirty);
                if dirty {
                    state
                        .0
                        .world
                        .insert_chunk(pos, dim.as_str(), chunk)
                        .expect("Failed to re-insert chunk after unloading from cache.");
                    written_chunks += 1;
                }
                unloaded_entries += 1;
            }
            None => {
                error!("Chunk at position {:?} could not be removed because it does not exist in the cache.", chunk_pos);
            }
        }
    }
    let remaining_chunks = state.0.world.get_cache().len();
    trace!(
        "Unloaded {} chunks from cache ({} written to world). {} chunks remain in cache.",
        unloaded_entries,
        written_chunks,
        remaining_chunks
    );
}
