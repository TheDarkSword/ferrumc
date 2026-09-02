use bevy_ecs::prelude::*;
use ferrumc_world::chunk::remap::NetworkBlockState;
use ferrumc_world::pos::BlockPos;
use std::time::{Duration, Instant};

use crate::BinaryError;
use ferrumc_attributes::Attributes;
use ferrumc_components::player::abilities::PlayerAbilities;
use ferrumc_components::player::gameplay_state::digging::PlayerDigging;
use ferrumc_core::transform::grounded::OnGround;
use ferrumc_data::attributes::Attribute;
use ferrumc_data::generated::block_properties;
use ferrumc_data::generated::effects::Effect;
use ferrumc_data::generated::items::Item;
use ferrumc_effects::ActiveEffects;
use ferrumc_inventories::hotbar::Hotbar;
use ferrumc_inventories::inventory::Inventory;
use ferrumc_messages::player_digging::*;
use ferrumc_mining::{ticks_to_break, tool_against, Digger};
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::{block_change_ack::BlockChangeAck, block_update::BlockUpdate};
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_state::GlobalStateResource;
use ferrumc_world::block_state_id::BlockStateId;
use tracing::{debug, error, warn};

/// How many ticks a second the server runs at, which is what a break time is counted in.
const TICKS_A_SECOND: f32 = 20.0;

// A query for just the components needed to acknowledge a dig packet
type DiggingPlayerQuery<'a> = (Entity, &'a StreamWriter, Option<&'a PlayerDigging>);

/// What is read off a digger to know how fast they work.
type Working<'a> = (
    &'a Inventory,
    &'a Hotbar,
    &'a OnGround,
    Option<&'a Attributes>,
    Option<&'a ActiveEffects>,
);

/// How fast the player works against this particular block.
///
/// What is in their hand, what it is worth against this block, what they are enchanted and dosed
/// with, and whether they are standing on anything.
fn digger_against(working: Option<Working>, block: &str) -> Digger {
    let Some((inventory, hotbar, grounded, attributes, effects)) = working else {
        return Digger::default();
    };

    let held = hotbar
        .get_selected_item(inventory)
        .ok()
        .flatten()
        .and_then(|slot| slot.item_id)
        .and_then(|id| u16::try_from(id.0 .0).ok())
        .and_then(Item::from_id);

    // Which blocks a tool's rule names is a tag the packs define, so it is asked of them rather
    // than matched here.
    let tags = ferrumc_registry::tags::current();
    let blocks = tags.block();
    let named = |rule: &str| match rule.strip_prefix('#') {
        Some(tag) => blocks
            .get_by_name(tag)
            .zip(ferrumc_registry::tags::protocol_id(
                "minecraft:block",
                block,
            ))
            .and_then(|(tag, id)| u32::try_from(id).ok().map(|id| (tag, id)))
            .is_some_and(|(tag, id)| blocks.contains(tag, id)),
        None => rule.strip_prefix("minecraft:").unwrap_or(rule) == block,
    };
    let (tool_speed, right_tool) = tool_against(held, named);

    Digger {
        tool_speed,
        right_tool,
        mining_efficiency: attributes
            .map_or(0.0, |a| a.value(&Attribute::MINING_EFFICIENCY) as f32),
        haste: effects
            .and_then(|held| held.level(Effect::Haste))
            .unwrap_or(0),
        fatigue: effects
            .and_then(|held| held.level(Effect::MiningFatigue))
            .unwrap_or(0),
        block_break_speed: attributes
            .map_or(1.0, |a| a.value(&Attribute::BLOCK_BREAK_SPEED) as f32),
        submerged_speed: attributes
            .map_or(0.2, |a| a.value(&Attribute::SUBMERGED_MINING_SPEED) as f32),
        // Whether a digger's head is under water needs the block at their eyes, which is the
        // damage system's question and not asked twice.
        eyes_in_water: false,
        on_ground: grounded.0,
    }
}

/// Handles the PlayerStartDiggingEvent.
/// This system starts the digging timer.
pub fn handle_start_digging(
    mut commands: Commands,
    mut events: MessageReader<PlayerStartedDigging>,
    mut player_query: Query<DiggingPlayerQuery, With<PlayerAbilities>>,
    working: Query<Working>,
    state: Res<GlobalStateResource>,
) {
    for event in events.read() {
        debug!(
            "Player {:?} started digging at {:?}",
            event.player, event.position
        );

        // --- 1. Get BlockStateId from the world ---
        let pos = event.position.clone().into();
        let block_state_id = match state.0.world.get_block_and_fetch(
            pos,
            "overworld", // TODO: remove hardcoded dimension
        ) {
            Ok(id) => id,
            Err(e) => {
                warn!(
                    "StartDigging: Failed to get block at {:?}: {:?}",
                    event.position, e
                );
                continue;
            }
        };
        // --- 2. Get Block Name ---
        let Some(block_name) =
            ferrumc_registry::lookup_blockstate_name(&VarInt::from(block_state_id).0.to_string())
        else {
            warn!("Could not find block name for state {:?}", block_state_id);
            continue;
        };

        // --- 3. Get Hardness ---
        // Read off the state rather than the block: a lit furnace and an unlit one are two states
        // and need not agree.
        let hardness = block_properties::hardness(block_state_id.raw());

        // --- 4. Check for unbreakable block ---
        if hardness < 0.0 {
            debug!(
                "Player {:?} tried to dig an unbreakable block ({})",
                event.player, block_name
            );

            // We must still send an ACK to the client.
            // But we do not add the PlayerDigging component.
            if let Ok((_, writer, _)) = player_query.get_mut(event.player) {
                let ack_packet = BlockChangeAck {
                    sequence: event.sequence,
                };
                if let Err(e) = writer.send_packet_ref(&ack_packet) {
                    error!(
                        "Failed to send start_dig ACK to {:?}: {:?}",
                        event.player, e
                    );
                }
            }
            continue; // Move to the next event
        }

        // --- 5. Calculate break time ---
        let bare = block_name
            .strip_prefix("minecraft:")
            .unwrap_or(block_name)
            .to_string();
        let digger = digger_against(working.get(event.player).ok(), &bare);
        let Some(ticks) = ticks_to_break(
            hardness,
            block_properties::needs_the_right_tool(block_state_id.raw()),
            &digger,
        ) else {
            debug!("Player {:?} cannot break {block_name} at all", event.player);
            continue;
        };
        let break_time = Duration::from_secs_f32(ticks as f32 / TICKS_A_SECOND);

        // --- 6. Add the component ----
        commands.entity(event.player).insert(PlayerDigging {
            block_pos: event.position.clone(),
            start_time: Instant::now(),
            break_time,
        });

        // --- 7. Acknowledge the client ---
        if let Ok((_, writer, _)) = player_query.get_mut(event.player) {
            let ack_packet = BlockChangeAck {
                sequence: event.sequence,
            };
            if let Err(e) = writer.send_packet_ref(&ack_packet) {
                error!(
                    "Failed to send start_dig ACK to {:?}: {:?}",
                    event.player, e
                );
            }
        }
    }
}

/// Handles the PlayerCancelDiggingEvent.
/// This system stops the digging timer.
pub fn handle_cancel_digging(
    mut commands: Commands,
    mut events: MessageReader<PlayerCancelledDigging>,
    mut player_query: Query<DiggingPlayerQuery>,
) {
    for event in events.read() {
        debug!("Player {:?} cancelled digging.", event.player);

        // Remove the component to stop the timer.
        commands.entity(event.player).remove::<PlayerDigging>();

        // Acknowledge the cancellation.
        if let Ok((_, writer, _)) = player_query.get_mut(event.player) {
            let ack_packet = BlockChangeAck {
                sequence: event.sequence,
            };
            if let Err(e) = writer.send_packet_ref(&ack_packet) {
                error!(
                    "Failed to send cancel_dig ACK to {:?}: {:?}",
                    event.player, e
                );
            }
        }
    }
}

/// Handles the PlayerFinishDiggingEvent.
/// This system checks the timer and breaks the block.
// A system's arguments are the state it needs: the world, the digger, who is watching, and what
// breaking costs them. Splitting it to shorten the list would only move the same state elsewhere.
#[expect(clippy::too_many_arguments)]
pub fn handle_finish_digging(
    mut commands: Commands,
    mut events: MessageReader<PlayerFinishedDigging>,
    state: Res<GlobalStateResource>,
    working: Query<Working>,
    mut player_query: Query<DiggingPlayerQuery>,
    broadcast_query: Query<(Entity, &StreamWriter)>, // For broadcasting the break
    mut block_break_writer: MessageWriter<ferrumc_messages::BlockBrokenEvent>,
    mut hunger: Query<&mut ferrumc_components::player::hunger::Hunger>,
) {
    for event in events.read() {
        // Breaking a block costs a little energy, which is most of why mining makes a player
        // hungry: a thousandth of a shank each, and a mine is a great many blocks.
        if let Ok(mut hunger) = hunger.get_mut(event.player) {
            hunger.spend(ferrumc_components::player::hunger::EXHAUSTION_MINE);
        }
        let Ok((_player_entity, writer, digging_opt)) = player_query.get_mut(event.player) else {
            warn!(
                "Player {:?} sent FinishDigging but query failed.",
                event.player
            );
            continue;
        };

        // Check if the player was actually digging
        let Some(digging) = digging_opt else {
            warn!(
                "Player {:?} finished digging without starting.",
                event.player
            );
            let ack_packet = BlockChangeAck {
                sequence: event.sequence,
            };
            if let Err(e) = writer.send_packet_ref(&ack_packet) {
                error!("Failed to send fail_dig ACK to {:?}: {:?}", event.player, e);
            }
            continue;
        };

        // --- 1. Validate the Dig ---
        if digging.block_pos != event.position {
            warn!(
                "Player {:?} finished digging the wrong block. (Expected {:?}, got {:?})",
                event.player, digging.block_pos, event.position
            );
            // Don't break the block, but still ACK
            let ack_packet = BlockChangeAck {
                sequence: event.sequence,
            };
            if let Err(e) = writer.send_packet_ref(&ack_packet) {
                error!("Failed to send fail_dig ACK to {:?}: {:?}", event.player, e);
            }
            commands.entity(event.player).remove::<PlayerDigging>();
            continue;
        }

        let elapsed = Instant::now().duration_since(digging.start_time);

        // --- 2. Check if enough time has passed ---
        if elapsed < digging.break_time {
            // --- ANTI-CHEAT ---
            warn!(
                "Player {:?} finished digging too fast! ({}ms < {}ms)",
                event.player,
                elapsed.as_millis(),
                digging.break_time.as_millis()
            );

            let pos = event.position.clone().into();
            let real_block_state = match state.0.world.get_block_and_fetch(pos, "overworld") {
                Ok(id) => id,
                Err(e) => {
                    error!(
                        "Failed to get real block state for anti-cheat revert: {:?}",
                        e
                    );
                    BlockStateId::default()
                }
            };

            let revert_packet = BlockUpdate {
                location: event.position.clone(),
                block_state_id: NetworkBlockState::from(real_block_state),
            };

            if let Err(e) = writer.send_packet_ref(&revert_packet) {
                error!(
                    "Failed to send anti-cheat revert packet to {:?}: {:?}",
                    event.player, e
                );
            }
        } else {
            // --- 3. SUCCESS: Break the Block ---
            debug!(
                "Player {:?} finished digging at {:?}",
                event.player, event.position
            );

            // We wrap the block-breaking logic in its own function
            // to handle the errors cleanly (replaces `try` block).
            // What was in hand when it broke, which is what the loot table asks about.
            let held = working
                .get(event.player)
                .ok()
                .and_then(|(inventory, hotbar, _, _, _)| {
                    hotbar.get_selected_item(inventory).ok().flatten()?.item_id
                });
            if let Err(e) = break_block(
                &state,
                &broadcast_query,
                &event.position,
                &mut block_break_writer,
                held,
            ) {
                error!("Error handling finished digging: {:?}", e);
            }
        }

        // --- 4. Acknowledge and Clean up (This now runs for *both* cases) ---
        let ack_packet = BlockChangeAck {
            sequence: event.sequence,
        };
        if let Err(e) = writer.send_packet_ref(&ack_packet) {
            error!(
                "Failed to send finish_dig ACK to {:?}: {:?}",
                event.player, e
            );
        }
        commands.entity(event.player).remove::<PlayerDigging>();
    }
}

/// Helper function to contain the block-breaking logic (replaces `try` block)
fn break_block(
    state: &Res<GlobalStateResource>,
    broadcast_query: &Query<(Entity, &StreamWriter)>,
    position: &ferrumc_net_codec::net_types::network_position::NetworkPosition,
    block_break_writer: &mut MessageWriter<ferrumc_messages::BlockBrokenEvent>,
    tool: Option<ferrumc_inventories::item::ItemID>,
) -> Result<(), BinaryError> {
    let pos: BlockPos = position.clone().into();
    let mut chunk = ferrumc_utils::world::load_or_generate_mut(&state.0, pos.chunk(), "overworld")
        .expect("Failed to load or generate chunk");
    // Read before it goes: what a block leaves behind depends on which it was.
    let was = chunk.get_block(pos.chunk_block_pos());
    chunk.set_block(pos.chunk_block_pos(), BlockStateId::default());

    // Send block broken event for un-grounding system
    debug!("Sending BlockBrokenEvent for block at {:?}", pos.pos);
    block_break_writer.write(ferrumc_messages::BlockBrokenEvent {
        position: pos,
        state: was,
        tool,
    });

    // Broadcast the block break to all players
    let block_update_packet = BlockUpdate {
        location: position.clone(),
        block_state_id: NetworkBlockState::from(BlockStateId::default()),
    };
    for (eid, conn) in broadcast_query {
        if !state.0.players.is_connected(eid) {
            continue;
        }
        conn.send_packet_ref(&block_update_packet)
            .map_err(BinaryError::Net)?;
    }
    Ok(())
}
