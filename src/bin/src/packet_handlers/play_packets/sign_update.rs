//! Writing on a sign.
//!
//! The client opens its own editor when a sign is placed and sends this when the player is done.
//! What arrives is four lines of plain text; what is stored is four components, because that is
//! what a line is — it can be coloured, translated or carry a click event, and a string says none
//! of it.

use bevy_ecs::prelude::{Query, Res};
use ferrumc_config::server_config::get_global_config;
use ferrumc_core::transform::position::Position;
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::block_entity_data::BlockEntityData as BlockEntityDataPacket;
use ferrumc_net::SignUpdateReceiver;
use ferrumc_net_codec::net_types::network_position::NetworkPosition;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_state::GlobalStateResource;
use ferrumc_text::TextComponentBuilder;
use ferrumc_world::block_entity::BlockEntityData;
use ferrumc_world::pos::BlockPos;
use tracing::{error, trace};

pub fn handle(
    receiver: Res<SignUpdateReceiver>,
    state: Res<GlobalStateResource>,
    players: Query<(&StreamWriter, &Position)>,
) {
    for (event, _) in receiver.0.try_iter() {
        let pos: BlockPos = event.position.into();
        let lines = [event.line_1, event.line_2, event.line_3, event.line_4];

        let Ok(mut chunk) =
            ferrumc_utils::world::load_or_generate_mut(&state.0, pos.chunk(), "overworld")
        else {
            error!("Could not load the chunk holding the sign at {}", pos);
            continue;
        };
        let Some(entity) = chunk.block_entity_mut(pos.chunk_block_pos()) else {
            trace!("A sign was written on at {} where there is none", pos);
            continue;
        };
        let kind = entity.kind;
        let BlockEntityData::Sign(sign) = &mut entity.data else {
            trace!(
                "A sign was written on at {} where the block is not one",
                pos
            );
            continue;
        };

        // A waxed sign is finished: vanilla refuses further edits rather than ignoring them
        // quietly, and so does this.
        if sign.waxed {
            continue;
        }
        let face = if event.is_front {
            &mut sign.front
        } else {
            &mut sign.back
        };
        for (index, line) in lines.into_iter().enumerate() {
            face.set_line(index, &TextComponentBuilder::new(line).build());
        }

        let packet = BlockEntityDataPacket {
            position: NetworkPosition {
                x: pos.pos.x,
                y: pos.pos.y as i16,
                z: pos.pos.z,
            },
            entity_type: VarInt::new(i32::from(kind)),
            nbt: entity.to_nbt(),
        };
        // The guard goes before anything is sent, so nothing else waiting on this chunk is held up.
        drop(chunk);

        let render_distance = get_global_config().chunk_render_distance as i32;
        let chunk_pos = pos.chunk();
        for (conn, player) in players.iter() {
            let player_chunk = player.chunk();
            if (chunk_pos.x() - player_chunk.x).abs() <= render_distance
                && (chunk_pos.z() - player_chunk.y).abs() <= render_distance
            {
                if let Err(err) = conn.send_packet_ref(&packet) {
                    error!("Failed to send a sign's text: {:?}", err);
                }
            }
        }
    }
}
