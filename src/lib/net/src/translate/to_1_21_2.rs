//! Everything 1.21.4 changed that a client on 1.21.2 does not read.

use super::Translated;
use crate::packets::outgoing::set_held_slot::SetHeldItem;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_net_codec::version::ProtocolVersion;
use std::io::Write;

/// The boundary this hop is about: everything below it predates 1.21.4's changes.
const NATIVE: ProtocolVersion = ProtocolVersion::V1_21_4;

/// 1.21.4 widened the held slot to a varint. Older clients read a single byte, and there are only
/// nine hotbar slots, so nothing is lost narrowing it back.
pub fn set_held_slot<W: Write>(
    packet: &SetHeldItem,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    Some((|| {
        super::packet_id!(writer, opts, "play", "set_held_slot")?;
        (packet.slot.0 as u8).encode(writer, &opts.nested())?;
        Ok(())
    })())
}
