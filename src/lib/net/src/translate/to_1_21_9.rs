//! Everything 1.21.11 changed that a client on 1.21.9 does not read.

use super::Translated;
use crate::packets::outgoing::initialize_border::InitializeBorder;
use crate::packets::outgoing::set_border_lerp_size::SetBorderLerpSize;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_net_codec::net_types::var_long::VarLong;
use ferrumc_net_codec::version::ProtocolVersion;
use std::io::Write;

/// The boundary this hop is about: everything below it predates 1.21.11's changes.
const NATIVE: ProtocolVersion = ProtocolVersion::V1_21_11;

/// Milliseconds in a tick. 1.21.11 started counting the border's move in ticks; older clients
/// count it in milliseconds.
const MILLIS_PER_TICK: i64 = 50;

fn as_millis(ticks: &VarLong) -> VarLong {
    VarLong::new(ticks.0.saturating_mul(MILLIS_PER_TICK))
}

/// The whole border, as an older client reads it.
pub fn initialize_border<W: Write>(
    packet: &InitializeBorder,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    Some((|| {
        super::packet_id!(writer, opts, "play", "initialize_border")?;
        packet.center_x.encode(writer, &opts.nested())?;
        packet.center_z.encode(writer, &opts.nested())?;
        packet.old_diameter.encode(writer, &opts.nested())?;
        packet.new_diameter.encode(writer, &opts.nested())?;
        as_millis(&packet.speed).encode(writer, &opts.nested())?;
        packet
            .portal_teleport_boundary
            .encode(writer, &opts.nested())?;
        packet.warning_blocks.encode(writer, &opts.nested())?;
        packet.warning_time.encode(writer, &opts.nested())?;
        Ok(())
    })())
}

/// The border starting to move, in the units an older client expects.
pub fn set_border_lerp_size<W: Write>(
    packet: &SetBorderLerpSize,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    Some((|| {
        super::packet_id!(writer, opts, "play", "set_border_lerp_size")?;
        packet.old_diameter.encode(writer, &opts.nested())?;
        packet.new_diameter.encode(writer, &opts.nested())?;
        as_millis(&packet.speed).encode(writer, &opts.nested())?;
        Ok(())
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrumc_net_codec::encode::Framing;

    fn encoded_for(version: ProtocolVersion) -> Vec<u8> {
        let packet = SetBorderLerpSize {
            old_diameter: 100.0,
            new_diameter: 50.0,
            speed: VarLong::new(20),
        };
        let mut buffer = Vec::new();
        packet
            .encode(&mut buffer, &NetEncodeOpts::new(Framing::None, version))
            .expect("encodes");
        buffer
    }

    fn varlong(value: i64) -> Vec<u8> {
        let mut buffer = Vec::new();
        VarLong::new(value)
            .encode(
                &mut buffer,
                &NetEncodeOpts::new(Framing::None, ProtocolVersion::CURRENT),
            )
            .expect("encodes");
        buffer
    }

    /// A border closing over a second is twenty ticks to a current client and a thousand
    /// milliseconds to an older one. Sending the tick count would close it fifty times too fast.
    ///
    /// The speed is the last field, so comparing the tail is enough.
    #[test]
    fn the_border_speed_changes_units() {
        let native = encoded_for(ProtocolVersion::CURRENT);
        let older = encoded_for(ProtocolVersion::V1_21_9);

        let ticks = varlong(20);
        let millis = varlong(1000);
        assert_eq!(native[native.len() - ticks.len()..], ticks[..]);
        assert_eq!(older[older.len() - millis.len()..], millis[..]);
    }
}
