//! Launch utilities for server initialization, chunk generation, and world import.

use crate::cli::ImportArgs;
use crate::errors::BinaryError;
use ferrumc_config::server_config::get_global_config;
use ferrumc_general_purpose::paths::get_root_path;
use ferrumc_state::player_list::PlayerList;
use ferrumc_state::{GlobalState, ServerState};
use ferrumc_threadpool::ThreadPool;
use ferrumc_world::pos::ChunkPos;
use ferrumc_world::World;
use ferrumc_world_gen::WorldGenerator;
use std::time::Instant;
use tracing::{error, info, warn};

/// Creates the initial server state with all required components.
pub fn create_state(start_time: Instant) -> Result<ServerState, BinaryError> {
    let world = World::new(&get_global_config().database.db_path);

    // A world keeps the seed it was generated with. Drawing a new one would reshape the terrain
    // beyond what has already been written, leaving a seam wherever generation resumes, so a
    // recorded seed wins over the configured one.
    let configured = get_global_config().world_seed;
    let seed = match world.seed()? {
        Some(recorded) => {
            if configured.is_some_and(|configured| configured != recorded) {
                warn!(
                    "Ignoring the configured world seed: this world was generated with {recorded}.                      Clear the world to generate it afresh."
                );
            }
            recorded
        }
        None => {
            let chosen = configured.unwrap_or_else(rand::random);
            world.set_seed(chosen)?;
            chosen
        }
    };
    info!("World generation seed: {seed}");

    Ok(ServerState {
        world,
        terrain_generator: WorldGenerator::new(seed),
        shut_down: false.into(),
        players: PlayerList::default(),
        thread_pool: ThreadPool::new(),
        start_time,
    })
}

/// Generates spawn chunks around the origin if they don't exist.
pub fn generate_spawn_chunks(state: GlobalState) -> Result<(), BinaryError> {
    info!("No overworld spawn chunk found, generating spawn chunks...");

    let start = Instant::now();
    let radius = get_global_config().chunk_render_distance as i32;

    // Collect all chunk coordinates to generate
    let chunks: Vec<(i32, i32)> = (-radius..=radius)
        .flat_map(|x| (-radius..=radius).map(move |z| (x, z)))
        .collect();

    let mut batch = state.thread_pool.batch();
    for (x, z) in chunks {
        let state_clone = state.clone();
        batch.execute(move || {
            let pos = ChunkPos::new(x, z);
            let chunk = state_clone.terrain_generator.generate_chunk(pos);

            match chunk {
                Ok(chunk) => {
                    if let Err(e) = state_clone.world.insert_chunk(pos, "overworld", chunk) {
                        error!("Error saving chunk ({}, {}): {:?}", x, z, e);
                    }
                }
                Err(e) => {
                    error!("Error generating chunk ({}, {}): {:?}", x, z, e);
                }
            }
        });
    }
    batch.wait();

    info!("Finished generating spawn chunks in {:?}", start.elapsed());
    Ok(())
}

/// Handles importing a world from an external source.
pub fn handle_import(import_args: ImportArgs) -> Result<(), BinaryError> {
    info!("Importing world...");

    let mut world = World::new(&get_global_config().database.db_path);

    let root_path = get_root_path();
    let mut import_path = root_path.join(&import_args.import_path);
    if import_path.is_relative() {
        import_path = root_path.join(import_path);
    }

    if let Err(e) = world.import(import_path, ThreadPool::new()) {
        error!("Could not import world: {}", e.to_string());
        return Err(BinaryError::Custom("Could not import world.".to_string()));
    }

    Ok(())
}
