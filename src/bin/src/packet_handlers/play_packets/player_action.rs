use crate::errors::BinaryError;
use bevy_ecs::prelude::{Entity, MessageWriter, Query, Res, ResMut};
use ferrumc_components::player::abilities::PlayerAbilities;
use ferrumc_messages::player_digging::*;
use ferrumc_messages::BlockBrokenEvent;
use ferrumc_world::chunk::remap::NetworkBlockState;

use crate::systems::block_world::WorldAccess;
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::block_change_ack::BlockChangeAck;
use ferrumc_net::packets::outgoing::block_update::BlockUpdate;
use ferrumc_net::packets::outgoing::light_update::LightUpdate;
use ferrumc_net::PlayerActionReceiver;
use ferrumc_net_codec::net_types::network_position::NetworkPosition;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_state::GlobalStateResource;
use ferrumc_world::chunk::light::network::NetworkLightData;
use ferrumc_world::neighbour_update::NeighbourUpdater;
use ferrumc_world::{block_state_id::BlockStateId, pos::BlockPos};
use tracing::{error, warn};

pub fn handle(
    receiver: Res<PlayerActionReceiver>,
    state: Res<GlobalStateResource>,
    mut fluid_scheduler: ResMut<crate::systems::fluids::FluidScheduler>,
    tick: Res<ferrumc_core::tick::TickCounter>,
    broadcast_query: Query<(Entity, &StreamWriter)>,
    player_query: Query<&PlayerAbilities>,
    (mut start_dig_events, mut cancel_dig_events, mut finish_dig_events, mut block_break_events): (
        MessageWriter<PlayerStartedDigging>,
        MessageWriter<PlayerCancelledDigging>,
        MessageWriter<PlayerFinishedDigging>,
        MessageWriter<BlockBrokenEvent>,
    ),
) {
    // https://minecraft.wiki/w/Minecraft_Wiki:Projects/wiki.vg_merge/Protocol?oldid=2773393#Player_Action
    for (event, trigger_eid) in receiver.0.try_iter() {
        // Get the player's abilities to check their gamemode
        let Ok(abilities) = player_query.get(trigger_eid) else {
            warn!(
                "PlayerAction: Player {:?} has no PlayerAbilities component",
                trigger_eid
            );
            continue;
        };

        let pos: BlockPos = event.location.clone().into();
        if abilities.creative_mode {
            // --- CREATIVE MODE LOGIC ---
            // Only instabreak (status 0) is relevant in creative.
            if event.status.0 == 0 {
                let res: Result<(), BinaryError> = try {
                    let mut chunk = ferrumc_utils::world::load_or_generate_mut(
                        &state.0,
                        pos.chunk(),
                        "overworld",
                    )
                    .expect("Failed to load or generate chunk");
                    // Read before it goes: what a block leaves behind depends on which it was.
                    let was = chunk.get_block(pos.chunk_block_pos());
                    chunk.set_block(pos.chunk_block_pos(), BlockStateId::default());
                    // The guard has to go before the neighbours are told: they read and write
                    // blocks of their own, and two guards on one shard deadlock the tick thread.
                    drop(chunk);

                    // What stood on this block, or leaned against it, finds out now.
                    let mut world = WorldAccess::new(&state.0, &mut fluid_scheduler.0, tick.get());
                    NeighbourUpdater::default().block_changed(
                        &mut world,
                        pos,
                        BlockStateId::default(),
                    );
                    let cascade = std::mem::take(&mut world.changed);

                    // Breaking a block lets light through, or takes a light away.
                    let lit = crate::systems::world_light::relight_around(&state.0, pos);

                    // Send block broken event for un-grounding system
                    block_break_events.write(BlockBrokenEvent {
                        position: pos,
                        state: was,
                        // Broken outright rather than dug, which is a creative player: the loot
                        // table is not rolled for one, so nothing asks what was in hand.
                        tool: None,
                    });

                    // Broadcast the change
                    for (eid, conn) in &broadcast_query {
                        if !state.0.players.is_connected(eid) {
                            continue;
                        }

                        let block_update_packet = BlockUpdate {
                            location: event.location.clone(),
                            block_state_id: NetworkBlockState::from(BlockStateId::default()),
                        };
                        conn.send_packet_ref(&block_update_packet)
                            .map_err(BinaryError::Net)?;

                        for &(changed_pos, changed_state) in &cascade {
                            let packet = BlockUpdate {
                                location: NetworkPosition {
                                    x: changed_pos.pos.x,
                                    y: changed_pos.pos.y as i16,
                                    z: changed_pos.pos.z,
                                },
                                block_state_id: NetworkBlockState::from(changed_state),
                            };
                            conn.send_packet_ref(&packet).map_err(BinaryError::Net)?;
                        }

                        for &chunk_pos in &lit {
                            let Ok(chunk) = ferrumc_utils::world::load_or_generate_mut(
                                &state.0,
                                chunk_pos,
                                "overworld",
                            ) else {
                                continue;
                            };
                            let light = LightUpdate {
                                chunk_x: VarInt::new(chunk_pos.x()),
                                chunk_z: VarInt::new(chunk_pos.z()),
                                light: NetworkLightData::from(&*chunk),
                            };
                            conn.send_packet_ref(&light).map_err(BinaryError::Net)?;
                        }

                        if eid == trigger_eid {
                            // Send ACK to the creative player
                            let ack_packet = BlockChangeAck {
                                sequence: event.sequence,
                            };
                            conn.send_packet_ref(&ack_packet)
                                .map_err(BinaryError::Net)?;
                        }
                    }
                };
                if res.is_err() {
                    error!("Error handling creative player action: {:?}", res);
                }
            }
        } else {
            // --- SURVIVAL MODE LOGIC ---
            // This handler's only job is to fire messages.
            match event.status.0 {
                0 => {
                    // Started digging
                    start_dig_events.write(PlayerStartedDigging {
                        player: trigger_eid,
                        position: event.location,
                        sequence: event.sequence,
                    });
                }
                1 => {
                    // Cancelled digging
                    cancel_dig_events.write(PlayerCancelledDigging {
                        player: trigger_eid,
                        sequence: event.sequence,
                    });
                }
                2 => {
                    // Finished digging
                    finish_dig_events.write(PlayerFinishedDigging {
                        player: trigger_eid,
                        position: event.location,
                        sequence: event.sequence,
                    });
                }
                _ => {} // Other statuses (drop item, etc.) are handled by different packets
            }
        }
    }
}
