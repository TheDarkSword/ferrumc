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
    // 1.21 adds a field of its own on top of this form, so the hop below gets first refusal.
    if let Some(older) = super::to_1_21::login_finished(packet, writer, opts) {
        return Some(older);
    }
    Some(write_login_finished(packet, writer, opts))
}

/// The game profile as 26.1 reads it. Hops below build on this rather than restating it.
pub(super) fn write_login_finished<W: std::io::Write>(
    packet: &LoginSuccessPacket<'_>,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Result<(), ferrumc_net_codec::encode::errors::NetEncodeError> {
    packet.uuid.encode(writer, &opts.nested())?;
    packet.username.encode(writer, &opts.nested())?;
    packet.properties.encode(writer, &opts.nested())?;
    Ok(())
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
    // 1.21 drops another field on top of this one. The derive calls a single function per packet,
    // so the newest boundary that changes it hands older clients on to the next hop down.
    if let Some(older) = super::to_1_21::login(packet, writer, opts) {
        return Some(older);
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

#[cfg(test)]
mod tests {
    use super::*;
    use ferrumc_net_codec::encode::Framing;

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
