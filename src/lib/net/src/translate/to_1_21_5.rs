//! Everything 1.21.6 changed that a client on 1.21.5 does not read.

use super::Translated;
use crate::packets::outgoing::change_difficulty::ChangeDifficulty;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_net_codec::version::ProtocolVersion;
use std::io::Write;

/// The boundary this hop is about: everything below it predates 1.21.6's changes.
const NATIVE: ProtocolVersion = ProtocolVersion::V1_21_6;

/// 1.21.6 widened the difficulty to a varint. There are four of them, so the byte an older client
/// reads holds it either way.
pub fn change_difficulty<W: Write>(
    packet: &ChangeDifficulty,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    Some((|| {
        super::packet_id!(writer, opts, "play", "change_difficulty")?;
        (packet.difficulty.0 as u8).encode(writer, &opts.nested())?;
        packet.locked.encode(writer, &opts.nested())?;
        Ok(())
    })())
}
