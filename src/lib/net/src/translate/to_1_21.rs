//! Everything 1.21.2 changed that a client on 1.21 does not read.

use super::Body;
use ferrumc_net_codec::version::ProtocolVersion;
use std::io::Write;

/// The boundary this hop is about: everything below it predates 1.21.2's changes.
pub(super) const NATIVE: ProtocolVersion = ProtocolVersion::V1_21_2;

/// 1.21.2 added a sea level to the play login. 1.21 reads the secure chat flag straight after the
/// portal cooldown, so leaving the varint in shifts everything that follows.
#[must_use]
pub fn login<W: Write>(body: Body<'_, W>, version: ProtocolVersion) -> Body<'_, W> {
    if version >= NATIVE {
        return body;
    }
    body.without("sea_level")
}

/// 1.21.2 dropped a strict error handling flag from the game profile. Clients that still read it
/// stall on the login otherwise, and newer ones behave as though it were set, so it is sent as such.
#[must_use]
pub fn login_finished<W: Write>(body: Body<'_, W>, version: ProtocolVersion) -> Body<'_, W> {
    if version >= NATIVE {
        return body;
    }
    body.field("strict_error_handling", &true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packets::outgoing::login_play::LoginPlayPacket;
    use ferrumc_net_codec::encode::{Framing, NetEncode, NetEncodeOpts};

    fn encoded_for(version: ProtocolVersion) -> Vec<u8> {
        let mut buffer = Vec::new();
        LoginPlayPacket::new(1, 0)
            .encode(&mut buffer, &NetEncodeOpts::new(Framing::None, version))
            .expect("encodes");
        buffer
    }

    fn length_for(version: ProtocolVersion) -> usize {
        encoded_for(version).len()
    }

    /// The sea level sits second from last, so dropping it has to leave what surrounds it alone.
    /// A hop that rebuilt the body instead of editing it could reorder or lose a field here and
    /// still produce the right length.
    #[test]
    fn only_the_sea_level_is_missing_from_the_oldest_form() {
        // The packet id leading each body differs per version by design, and comes from the
        // generated tables rather than from a hop.
        let middle = &encoded_for(ProtocolVersion::V26_1)[1..];
        let oldest = &encoded_for(ProtocolVersion::V1_21)[1..];
        let removed = middle.len() - oldest.len();
        // Everything after the sea level: the secure chat flag.
        let tail = 1;

        assert_eq!(
            oldest[..oldest.len() - tail],
            middle[..middle.len() - tail - removed],
            "everything before the sea level should be untouched"
        );
        assert_eq!(
            oldest[oldest.len() - tail..],
            middle[middle.len() - tail..],
            "the secure chat flag should survive the field being cut from in front of it"
        );
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
