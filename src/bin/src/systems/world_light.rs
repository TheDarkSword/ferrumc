//! Light across the whole world rather than one chunk.
//!
//! The engines work through this so light crosses a chunk border, and so a block placed or broken
//! relights what it changed. Chunks that are not loaded are left alone: lighting must not pull a
//! chunk into memory, and what crosses that border is settled whenever the neighbour is loaded.

use ferrumc_config::server_config::get_global_config;
use ferrumc_core::transform::position::Position;
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::light_update::LightUpdate;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_state::GlobalState;
use ferrumc_world::block_state_id::BlockStateId;
use ferrumc_world::chunk::light::network::NetworkLightData;
use ferrumc_world::light::{LightEngine, LightLayer, LightWorld};
use ferrumc_world::pos::{BlockPos, ChunkPos};
use std::collections::HashSet;
use tracing::error;

pub struct WorldLight<'a> {
    state: &'a GlobalState,
    /// Chunks whose light changed, so the players who can see them can be told.
    pub touched: HashSet<ChunkPos>,
}

impl<'a> WorldLight<'a> {
    pub fn new(state: &'a GlobalState) -> Self {
        Self {
            state,
            touched: HashSet::new(),
        }
    }

    fn loaded(&self, chunk: ChunkPos) -> bool {
        self.state
            .world
            .chunk_exists(chunk, "overworld")
            .unwrap_or(false)
    }
}

impl LightWorld for WorldLight<'_> {
    fn block_at(&mut self, pos: BlockPos) -> BlockStateId {
        match ferrumc_utils::world::load_or_generate_mut(self.state, pos.chunk(), "overworld") {
            Ok(chunk) => chunk.get_block(pos.chunk_block_pos()),
            Err(err) => {
                error!("Could not read the block at {}: {:?}", pos, err);
                BlockStateId::new(0)
            }
        }
    }

    fn light_at(&mut self, pos: BlockPos, layer: LightLayer) -> u8 {
        match ferrumc_utils::world::load_or_generate_mut(self.state, pos.chunk(), "overworld") {
            Ok(chunk) => match layer {
                LightLayer::Block => chunk.block_light(pos.chunk_block_pos()),
                LightLayer::Sky => chunk.sky_light(pos.chunk_block_pos()),
            },
            Err(_) => 0,
        }
    }

    fn set_light(&mut self, pos: BlockPos, layer: LightLayer, level: u8) {
        match ferrumc_utils::world::load_or_generate_mut(self.state, pos.chunk(), "overworld") {
            Ok(mut chunk) => {
                match layer {
                    LightLayer::Block => chunk.set_block_light(pos.chunk_block_pos(), level),
                    LightLayer::Sky => chunk.set_sky_light(pos.chunk_block_pos(), level),
                }
                self.touched.insert(pos.chunk());
            }
            Err(err) => error!("Could not light {}: {:?}", pos, err),
        }
    }

    fn height_range(&self) -> (i32, i32) {
        (-64, 320)
    }

    fn stores_light(&mut self, pos: BlockPos) -> bool {
        // Within the world's height, and only where a chunk is already loaded: lighting a chunk
        // must not be what causes its neighbours to be generated.
        (-64..320).contains(&pos.pos.y) && self.loaded(pos.chunk())
    }
}

/// Relights around a block that changed, and says which chunks that touched.
///
/// Both kinds are worked out. Block light needs only the position that changed; sky light may need
/// the whole column, since a block placed under the sky darkens everything below it and one broken
/// opens the column up again.
pub fn relight_around(state: &GlobalState, pos: BlockPos) -> HashSet<ChunkPos> {
    let mut world = WorldLight::new(state);

    let mut block = LightEngine::for_layer(LightLayer::Block);
    block.check(pos);
    block.run(&mut world);

    let mut sky = LightEngine::for_layer(LightLayer::Sky);
    sky.check_sky_column(&mut world, pos);
    sky.run(&mut world);

    world.touched
}

/// Relights around a block that changed and tells everyone in range about the chunks it touched.
pub fn relight_and_send<'a>(
    state: &GlobalState,
    pos: BlockPos,
    players: impl Iterator<Item = (&'a StreamWriter, &'a Position)>,
) {
    let touched = relight_around(state, pos);
    if touched.is_empty() {
        return;
    }

    let render_distance = get_global_config().chunk_render_distance as i32;
    let players: Vec<_> = players.collect();
    for chunk_pos in touched {
        let Ok(chunk) = ferrumc_utils::world::load_or_generate_mut(state, chunk_pos, "overworld")
        else {
            continue;
        };
        let packet = LightUpdate {
            chunk_x: VarInt::new(chunk_pos.x()),
            chunk_z: VarInt::new(chunk_pos.z()),
            light: NetworkLightData::from(&*chunk),
        };
        for (conn, player) in &players {
            let player_chunk = player.chunk();
            if (chunk_pos.x() - player_chunk.x).abs() <= render_distance
                && (chunk_pos.z() - player_chunk.y).abs() <= render_distance
            {
                if let Err(err) = conn.send_packet_ref(&packet) {
                    error!("Failed to send a light update: {:?}", err);
                }
            }
        }
    }
}

/// Lets light flow across a chunk's borders once its neighbours are there.
///
/// A chunk is lit on its own when it is generated, because lighting must not be what pulls its
/// neighbours into memory. What that leaves wrong is only the spreading across the border, and only
/// where one side is brighter than the other, so that is what is looked for: a border position
/// whose neighbour across the line is bright enough to reach it is given that light to spread from.
///
/// Returns the chunks whose light changed.
pub fn pull_light_across_borders(state: &GlobalState, chunk_pos: ChunkPos) -> HashSet<ChunkPos> {
    let mut world = WorldLight::new(state);
    let (bottom, top) = world.height_range();

    let mut block = LightEngine::for_layer(LightLayer::Block);
    let mut sky = LightEngine::for_layer(LightLayer::Sky);
    let mut any = false;

    // The lines of blocks either side of each border. Light has to cross both ways: the new chunk
    // has never been told what is next to it, and a torch in it has never reached out.
    let base_x = chunk_pos.x() * 16;
    let base_z = chunk_pos.z() * 16;
    let mut borders = Vec::with_capacity(128);
    for i in 0..16 {
        borders.push((base_x + i, base_z - 1));
        borders.push((base_x + i, base_z));
        borders.push((base_x + i, base_z + 16));
        borders.push((base_x + i, base_z + 15));
        borders.push((base_x - 1, base_z + i));
        borders.push((base_x, base_z + i));
        borders.push((base_x + 16, base_z + i));
        borders.push((base_x + 15, base_z + i));
    }

    for (x, z) in borders {
        let column = BlockPos::of(x, 0, z);
        if !world.loaded(column.chunk()) {
            continue;
        }
        for y in bottom..top {
            let at = BlockPos::of(x, y, z);
            let block_level = world.light_at(at, LightLayer::Block);
            if block_level > 1 {
                block.push_from(at, block_level);
                any = true;
            }
            let sky_level = world.light_at(at, LightLayer::Sky);
            if sky_level > 1 {
                sky.push_from(at, sky_level);
                any = true;
            }
        }
    }

    if !any {
        return HashSet::new();
    }
    block.run(&mut world);
    sky.run(&mut world);
    world.touched
}
