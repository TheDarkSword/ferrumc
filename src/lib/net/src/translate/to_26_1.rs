//! Everything 26.2 added that a client on 26.1 or older does not read.

use super::{Body, Translated};
use crate::packets::outgoing::login_play::LoginPlayPacket;
use crate::packets::outgoing::login_success::LoginSuccessPacket;
use ferrumc_net_codec::encode::NetEncodeOpts;
use ferrumc_net_codec::version::ProtocolVersion;
use std::io::Write;

/// The boundary this hop is about: everything below it predates 26.2's additions.
pub(super) const NATIVE: ProtocolVersion = ProtocolVersion::V26_2;

/// 26.2 appended a session id to the game profile. Sending those sixteen bytes to an older client
/// leaves it reading a field that is not there, and the login exchange stalls with nothing logged.
pub fn login_finished<W: Write>(
    packet: &LoginSuccessPacket<'_>,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    if let Err(err) = super::packet_id!(writer, opts, "login", "login_finished") {
        return Some(Err(err));
    }
    let body = Body::new()
        .field("uuid", &packet.uuid)
        .field("username", &packet.username)
        .field("properties", &packet.properties);
    // 1.21 reads one more field after these.
    Some(super::to_1_21::login_finished(body, opts.version).write(writer, opts))
}

/// 26.2 added an online-mode flag ahead of the secure chat flag. An older client reads the two as
/// one, and everything after the play login desynchronises.
pub fn login<W: Write>(
    packet: &LoginPlayPacket<'_>,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    if let Err(err) = super::packet_id!(writer, opts, "play", "login") {
        return Some(Err(err));
    }
    let body = Body::new()
        .field("entity_id", &packet.entity_id)
        .field("is_hardcore", &packet.is_hardcore)
        .field("dimension_length", &packet.dimension_length)
        .field("dimension_names", &packet.dimension_names)
        .field("max_players", &packet.max_players)
        .field("view_distance", &packet.view_distance)
        .field("simulation_distance", &packet.simulation_distance)
        .field("reduced_debug_info", &packet.reduced_debug_info)
        .field("enable_respawn_screen", &packet.enable_respawn_screen)
        .field("do_limited_crafting", &packet.do_limited_crafting)
        .field("dimension_type", &packet.dimension_type)
        .field("dimension_name", &packet.dimension_name)
        .field("seed_hash", &packet.seed_hash)
        .field("gamemode", &packet.gamemode)
        .field("previous_gamemode", &packet.previous_gamemode)
        .field("is_debug", &packet.is_debug)
        .field("is_flat", &packet.is_flat)
        .field("has_death_location", &packet.has_death_location)
        .field("death_dimension_name", &packet.death_dimension_name)
        .field("death_location", &packet.death_location)
        .field("portal_cooldown", &packet.portal_cooldown)
        .field("sea_level", &packet.sea_level)
        // 26.2's online_mode goes here and is left out.
        .field("enforces_secure_chat", &packet.enforces_secure_chat);
    Some(super::to_1_21::login(body, opts.version).write(writer, opts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrumc_net_codec::encode::{Framing, NetEncode};

    fn encoded_for(version: ProtocolVersion) -> Vec<u8> {
        let packet = LoginPlayPacket::new(1, 0);
        let mut buffer = Vec::new();
        packet
            .encode(&mut buffer, &NetEncodeOpts::new(Framing::None, version))
            .expect("encodes");
        buffer
    }

    /// 26.2 writes an online-mode boolean the older form leaves out. Without the hop an older
    /// client reads that byte as the secure chat flag and everything after it shifts.
    #[test]
    fn online_mode_is_written_only_for_26_2() {
        let native = encoded_for(ProtocolVersion::V26_2);
        let older = encoded_for(ProtocolVersion::V26_1);

        assert_eq!(
            native.len(),
            older.len() + 1,
            "the 26.2 form should be exactly one boolean longer than the 26.1 form"
        );
        assert_eq!(
            native[..older.len() - 1],
            older[..older.len() - 1],
            "everything before the added flag should be identical"
        );
    }

    /// Everything from 1.21.2 up to 26.1 reads the same body. 1.21 drops one more field, which
    /// `to_1_21` handles.
    #[test]
    fn versions_between_the_two_boundaries_share_a_form() {
        let older = encoded_for(ProtocolVersion::V26_1);
        for version in ProtocolVersion::ALL {
            if version >= ProtocolVersion::V26_2 || version < ProtocolVersion::V1_21_2 {
                continue;
            }
            assert_eq!(
                encoded_for(version).len(),
                older.len(),
                "{version} should get the same body as 26.1"
            );
        }
        assert!(
            encoded_for(ProtocolVersion::V1_21).len() < older.len(),
            "1.21 is missing the sea level as well"
        );
    }
}
