//! Everything 26.2 added that a client on 26.1 or older does not read.

use super::Translated;
use crate::packets::outgoing::login_play::LoginPlayPacket;
use crate::packets::outgoing::login_success::LoginSuccessPacket;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_net_codec::version::ProtocolVersion;

/// The boundary this hop is about: everything below it predates 26.2's additions.
const NATIVE: ProtocolVersion = ProtocolVersion::V26_2;

/// 26.2 appended a session id to the game profile. Sending those sixteen bytes to an older client
/// leaves it reading a field that is not there, and the login exchange stalls with nothing logged.
pub fn login_finished<W: std::io::Write>(
    packet: &LoginSuccessPacket<'_>,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    Some((|| {
        packet.uuid.encode(writer, &opts.nested())?;
        packet.username.encode(writer, &opts.nested())?;
        packet.properties.encode(writer, &opts.nested())?;
        Ok(())
    })())
}

/// 26.2 added an online-mode flag ahead of the secure chat flag. An older client reads the two as
/// one, and everything after the play login desynchronises.
pub fn login<W: std::io::Write>(
    packet: &LoginPlayPacket<'_>,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    Some((|| {
        packet.entity_id.encode(writer, &opts.nested())?;
        packet.is_hardcore.encode(writer, &opts.nested())?;
        packet.dimension_length.encode(writer, &opts.nested())?;
        packet.dimension_names.encode(writer, &opts.nested())?;
        packet.max_players.encode(writer, &opts.nested())?;
        packet.view_distance.encode(writer, &opts.nested())?;
        packet.simulation_distance.encode(writer, &opts.nested())?;
        packet.reduced_debug_info.encode(writer, &opts.nested())?;
        packet
            .enable_respawn_screen
            .encode(writer, &opts.nested())?;
        packet.do_limited_crafting.encode(writer, &opts.nested())?;
        packet.dimension_type.encode(writer, &opts.nested())?;
        packet.dimension_name.encode(writer, &opts.nested())?;
        packet.seed_hash.encode(writer, &opts.nested())?;
        packet.gamemode.encode(writer, &opts.nested())?;
        packet.previous_gamemode.encode(writer, &opts.nested())?;
        packet.is_debug.encode(writer, &opts.nested())?;
        packet.is_flat.encode(writer, &opts.nested())?;
        packet.has_death_location.encode(writer, &opts.nested())?;
        packet.death_dimension_name.encode(writer, &opts.nested())?;
        packet.death_location.encode(writer, &opts.nested())?;
        packet.portal_cooldown.encode(writer, &opts.nested())?;
        packet.sea_level.encode(writer, &opts.nested())?;
        // 26.2's online_mode goes here and is left out.
        packet.enforces_secure_chat.encode(writer, &opts.nested())?;
        Ok(())
    })())
}
