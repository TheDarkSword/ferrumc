use bevy_ecs::prelude::{Entity, Query, Res, ResMut};
use ferrumc_core::collisions::bounds::CollisionBounds;
use ferrumc_core::tick::TickCounter;
use ferrumc_core::transform::position::Position;
use ferrumc_core::transform::rotation::Rotation;
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::block_change_ack::BlockChangeAck;
use ferrumc_net::packets::outgoing::block_update::BlockUpdate;
use ferrumc_net::PlaceBlockReceiver;
use ferrumc_net_codec::net_types::network_position::NetworkPosition;
use ferrumc_state::GlobalStateResource;
use ferrumc_world::chunk::remap::NetworkBlockState;
use ferrumc_world::pos::BlockPos;
use tracing::{debug, error, trace};

use crate::systems::fluids::{seed_fluid_tick, ActiveDimension, FluidScheduler};

use ferrumc_config::server_config::get_global_config;
use ferrumc_core::mq;
use ferrumc_inventories::hotbar::Hotbar;
use ferrumc_inventories::inventory::Inventory;
use ferrumc_text::{Color, NamedColor, TextComponentBuilder};
use ferrumc_world::block_behaviour::{behaviour_at, BlockWorld, InteractionResult, Use};
use ferrumc_world::block_state::Direction;
use ferrumc_world::block_state_id::BlockStateId;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::str::FromStr;

const ITEM_TO_BLOCK_MAPPING_FILE: &str =
    include_str!("../../../../../assets/data/item_to_block_mapping.json");
static ITEM_TO_BLOCK_MAPPING: Lazy<HashMap<i32, BlockStateId>> = Lazy::new(|| {
    let str_form: HashMap<String, String> = serde_json::from_str(ITEM_TO_BLOCK_MAPPING_FILE)
        .expect("Failed to parse item_to_block_mapping.json");
    str_form
        .into_iter()
        .map(|(k, v)| {
            (
                i32::from_str(&k).unwrap(),
                BlockStateId::new(u32::from_str(&v).unwrap()),
            )
        })
        .collect()
});

pub fn handle(
    receiver: Res<PlaceBlockReceiver>,
    state: Res<GlobalStateResource>,
    query: Query<(
        Entity,
        &StreamWriter,
        &Inventory,
        &Hotbar,
        &Position,
        &Rotation,
    )>,
    pos_q: Query<(&Position, &CollisionBounds)>,
    mut fluid_scheduler: ResMut<FluidScheduler>,
    dim: Res<ActiveDimension>,
    tick: Res<TickCounter>,
) {
    'ev_loop: for (event, eid) in receiver.0.try_iter() {
        let Ok((entity, conn, inventory, hotbar, _, rotation)) = query.get(eid) else {
            debug!("Could not get connection for entity {:?}", eid);
            continue;
        };
        if !state.0.players.is_connected(entity) {
            trace!("Entity {:?} is not connected", entity);
            continue;
        }
        match event.hand.0 {
            0 => {
                // Vanilla gives the block the first say: a door opens rather than a block being
                // placed against it. Only a sneaking player with something in hand skips this, and
                // whether a player is sneaking is not tracked yet.
                let clicked: BlockPos = event.position.into();
                let mut world = UsedBlocks::new(&state.0);
                let clicked_state = world.block_at(clicked);
                if let Some(behaviour) = behaviour_at(clicked_state) {
                    let mut ctx = Use {
                        world: &mut world,
                        pos: clicked,
                        player_facing: Direction::from_yaw(rotation.yaw),
                    };
                    if behaviour.use_without_item(clicked_state, &mut ctx)
                        == InteractionResult::Success
                    {
                        let changed = world.changed;
                        broadcast_changes(&changed, &query);
                        if let Err(err) = conn.send_packet_ref(&BlockChangeAck {
                            sequence: event.sequence,
                        }) {
                            error!("Failed to acknowledge block use: {:?}", err);
                        }
                        continue 'ev_loop;
                    }
                }

                let Ok(slot) = hotbar.get_selected_item(inventory) else {
                    error!("Could not fetch {:?}", eid);
                    continue 'ev_loop;
                };
                if let Some(selected_item) = slot {
                    let Some(item_id) = selected_item.item_id else {
                        error!("Selected item has no item ID");
                        continue 'ev_loop;
                    };
                    let Some(mapped_block_state_id) = ITEM_TO_BLOCK_MAPPING.get(&item_id.0 .0)
                    else {
                        error!("No block mapping found for item ID: {}", item_id.0);
                        continue 'ev_loop;
                    };
                    debug!(
                        "Placing block with item ID: {}, mapped to block state ID: {}",
                        item_id.0, mapped_block_state_id
                    );
                    let pos: BlockPos = clicked;
                    if pos.pos.y >= 319 {
                        mq::queue(
                            TextComponentBuilder::new(
                                "Build limit is 319! Cannot place block here.".to_string(),
                            )
                            .color(Color::Named(NamedColor::Red))
                            .bold()
                            .build(),
                            true,
                            entity,
                        );
                        trace!("Block placement out of bounds: {}", pos);
                        continue 'ev_loop;
                    } else if pos.pos.y <= -64 {
                        mq::queue(
                            TextComponentBuilder::new(
                                "Cannot place block below Y=-64.".to_string(),
                            )
                            .color(Color::Named(NamedColor::Red))
                            .bold()
                            .build(),
                            true,
                            entity,
                        );
                        trace!("Block placement out of bounds: {}", pos);
                        continue 'ev_loop;
                    }
                    let offset_pos = pos
                        + match event.face.0 {
                            0 => (0, -1, 0),
                            1 => (0, 1, 0),
                            2 => (0, 0, -1),
                            3 => (0, 0, 1),
                            4 => (-1, 0, 0),
                            5 => (1, 0, 0),
                            _ => (0, 0, 0),
                        };

                    let mut chunk = ferrumc_utils::world::load_or_generate_mut(
                        &state.0,
                        offset_pos.chunk(),
                        "overworld",
                    )
                    .expect("Failed to load or generate chunk");
                    let block_clicked = chunk.get_block(offset_pos.chunk_block_pos());
                    trace!("Block clicked: {:?}", block_clicked);

                    // Check if the block collides with any entities
                    let does_collide = {
                        pos_q.into_iter().any(|(pos, bounds)| {
                            bounds.collides(
                                (pos.x, pos.y, pos.z),
                                &CollisionBounds {
                                    x_offset_start: 0.0,
                                    x_offset_end: 1.0,
                                    y_offset_start: 0.0,
                                    y_offset_end: 1.0,
                                    z_offset_start: 0.0,
                                    z_offset_end: 1.0,
                                },
                                (
                                    offset_pos.pos.x as f64,
                                    offset_pos.pos.y as f64,
                                    offset_pos.pos.z as f64,
                                ),
                            )
                        })
                    };

                    if does_collide {
                        trace!("Block placement collided with entity");
                        continue 'ev_loop;
                    }

                    chunk.set_block(offset_pos.chunk_block_pos(), *mapped_block_state_id);

                    // Release the chunk write guard before seeding fluid ticks. `chunk` is a
                    // DashMap RefMut holding the shard lock; because it implements Drop it would
                    // otherwise stay alive until the end of this scope. `seed_fluid_tick` loads
                    // the same chunk again, which would re-enter the same shard lock and deadlock
                    // the tick thread. Multi-thread is so hard :3
                    drop(chunk);

                    let ack_packet = BlockChangeAck {
                        sequence: event.sequence,
                    };

                    let chunk_packet = BlockUpdate {
                        location: NetworkPosition {
                            x: offset_pos.pos.x,
                            y: offset_pos.pos.y as i16,
                            z: offset_pos.pos.z,
                        },
                        block_state_id: NetworkBlockState::from(*mapped_block_state_id),
                    };

                    // Broadcast the authoritative block state to every nearby player FIRST, then
                    // acknowledge the placing client's sequence LAST. Order matters: on block
                    // placement the client predicts the block locally and, when it receives the
                    // BlockChangeAck, reconciles that prediction against the authoritative state it
                    // currently knows. If the ack arrives before the BlockUpdate, the client sees
                    // the cell as still empty, discards its prediction (the block flickers out),
                    // then the BlockUpdate arrives and it reappears. Sending the update before the
                    // ack keeps the client's authoritative state in sync at reconcile time, so the
                    // placed block never flickers. (The digging path already sends update-then-ack.)
                    let offset_chunk = offset_pos.chunk();
                    let (offset_chunk_x, offset_chunk_z) = (offset_chunk.x(), offset_chunk.z());
                    let render_distance = get_global_config().chunk_render_distance as i32;
                    for (_, conn, _, _, pos, _) in query.iter() {
                        let chunk = pos.chunk();
                        let (chunk_x, chunk_z) = (chunk.x, chunk.y);

                        // Only send block update if the player is within the render distance of the block being updated
                        if (offset_chunk_x - chunk_x).abs() <= render_distance
                            && (offset_chunk_z - chunk_z).abs() <= render_distance
                        {
                            if let Err(err) = conn.send_packet_ref(&chunk_packet) {
                                error!("Failed to send block update packet: {:?}", err);
                            }
                        }
                    }

                    if let Err(err) = conn.send_packet_ref(&ack_packet) {
                        error!("Failed to send block change ack packet: {:?}", err);
                        continue 'ev_loop;
                    }

                    // Seed fluid simulation. If the placed block is itself a fluid it will begin
                    // to spread; in all cases, neighbouring fluids may need to react to the new
                    // block (e.g. flow around or be blocked by it). The chunk borrow above is
                    // released before seeding because seeding loads chunks itself.
                    let current_tick = tick.get();
                    let scheduler = &mut fluid_scheduler.0;
                    let dim = *dim;
                    seed_fluid_tick(scheduler, &state.0, dim, current_tick, offset_pos);
                    for neighbour in [
                        offset_pos + (0, 1, 0),
                        offset_pos + (0, -1, 0),
                        offset_pos + (1, 0, 0),
                        offset_pos + (-1, 0, 0),
                        offset_pos + (0, 0, 1),
                        offset_pos + (0, 0, -1),
                    ] {
                        seed_fluid_tick(scheduler, &state.0, dim, current_tick, neighbour);
                    }
                }
            }
            1 => {
                trace!("Offhand block placement not implemented");
            }
            _ => {
                debug!("Invalid hand");
            }
        }
    }
}

/// The world as a block behaviour sees it, remembering what it changed so the changes can be sent.
///
/// Each read and write takes the chunk guard and gives it back before the next one: holding two at
/// once on the same shard deadlocks the tick thread.
struct UsedBlocks<'a> {
    state: &'a ferrumc_state::GlobalState,
    changed: Vec<(BlockPos, BlockStateId)>,
}

impl<'a> UsedBlocks<'a> {
    fn new(state: &'a ferrumc_state::GlobalState) -> Self {
        Self {
            state,
            changed: Vec::new(),
        }
    }
}

impl BlockWorld for UsedBlocks<'_> {
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
}

/// Tells everyone in range about blocks a behaviour changed.
fn broadcast_changes(
    changed: &[(BlockPos, BlockStateId)],
    query: &Query<(
        Entity,
        &StreamWriter,
        &Inventory,
        &Hotbar,
        &Position,
        &Rotation,
    )>,
) {
    let render_distance = get_global_config().chunk_render_distance as i32;
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
        for (_, conn, _, _, player, _) in query.iter() {
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
