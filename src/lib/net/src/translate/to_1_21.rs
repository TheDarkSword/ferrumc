//! Everything 1.21.2 changed that a client on 1.21 does not read.

use super::Translated;
use crate::packets::outgoing::login_play::LoginPlayPacket;
use crate::packets::outgoing::login_success::LoginSuccessPacket;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_net_codec::version::ProtocolVersion;
use std::io::Write;

/// The boundary this hop is about: everything below it predates 1.21.2's changes.
const NATIVE: ProtocolVersion = ProtocolVersion::V1_21_2;

/// 1.21.2 added a sea level to the play login. 1.21 reads the secure chat flag straight after the
/// portal cooldown, so an extra varint there shifts everything that follows.
pub fn login<W: Write>(
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
        // 1.21.2's sea level and 26.2's online mode both go here and are left out.
        packet.enforces_secure_chat.encode(writer, &opts.nested())?;
        Ok(())
    })())
}

/// 1.21.2 dropped a strict error handling flag from the game profile. Clients that still read it
/// stall on the login otherwise, and newer ones behave as though it were set, so it is sent as such.
pub fn login_finished<W: Write>(
    packet: &LoginSuccessPacket<'_>,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    Some((|| {
        super::to_26_1::write_login_finished(packet, writer, opts)?;
        true.encode(writer, &opts.nested())?;
        Ok(())
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrumc_net_codec::encode::Framing;

    fn length_for(version: ProtocolVersion) -> usize {
        let mut buffer = Vec::new();
        LoginPlayPacket::new(1, 0)
            .encode(&mut buffer, &NetEncodeOpts::new(Framing::None, version))
            .expect("encodes");
        buffer.len()
    }

    /// 1.21 drops the sea level varint that 1.21.2 added, on top of 26.2's online mode.
    #[test]
    fn the_oldest_form_drops_both_added_fields() {
        let newest = length_for(ProtocolVersion::CURRENT);
        let middle = length_for(ProtocolVersion::V26_1);
        let oldest = length_for(ProtocolVersion::V1_21);

        assert_eq!(newest, middle + 1, "26.2 adds one boolean over 26.1");
        assert!(oldest < middle, "1.21 should also be missing the sea level");
    }

    /// 1.21.2 and above keep the sea level.
    #[test]
    fn the_boundary_is_1_21_2() {
        assert_eq!(
            length_for(ProtocolVersion::V1_21_2),
            length_for(ProtocolVersion::V26_1)
        );
        assert!(length_for(ProtocolVersion::V1_21) < length_for(ProtocolVersion::V1_21_2));
    }
}
