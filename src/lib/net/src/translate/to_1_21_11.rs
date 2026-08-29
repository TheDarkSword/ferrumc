//! Everything 26.1 changed that a client on 1.21.11 or older does not read.

use super::{Translated, Upgrade, Upgraded};
use crate::packets::outgoing::update_time::UpdateTimePacket;
use ferrumc_net_codec::decode::errors::NetDecodeError;
use ferrumc_net_codec::decode::{NetDecode, NetDecodeOpts};
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_net_codec::net_types::lp_vec3::LowPrecisionVec3;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::version::ProtocolVersion;
use std::io::Read;
use std::io::Write;

/// The boundary this hop is about: everything below it predates 26.1's changes.
const NATIVE: ProtocolVersion = ProtocolVersion::V26_1;

/// Index of `minecraft:overworld` in the `minecraft:world_clock` registry, which only exists from
/// 26.1. Older clients have no such registry and take the overworld's time directly.
const OVERWORLD_CLOCK: i32 = 0;

/// 26.1 replaced the time of day with a map of world clocks. Older clients read a game time, a day
/// time, and whether the cycle is running — so the overworld clock is unpacked back into those.
pub fn set_time<W: Write>(
    packet: &UpdateTimePacket,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    if let Err(err) = super::packet_id!(writer, opts, "play", "set_time") {
        return Some(Err(err));
    }

    let overworld = packet
        .clock_updates
        .data
        .iter()
        .find(|update| update.clock.0 == OVERWORLD_CLOCK);

    Some((|| {
        packet.game_time.encode(writer, &opts.nested())?;
        // With no clock to read from, the day time stands still rather than jumping.
        let (day_time, advancing) = overworld
            .map(|clock| (clock.total_ticks.0, clock.rate != 0.0))
            .unwrap_or((0, false));
        // 1.21 has no flag of its own, so it takes the pair and gives back what it does read.
        let (day_time, advancing) = super::to_1_21::set_time(day_time, advancing, opts.version);
        day_time.encode(writer, &opts.nested())?;
        if let Some(advancing) = advancing {
            advancing.encode(writer, &opts.nested())?;
        }
        Ok(())
    })())
}

/// What the action field meant before 26.1 split the packet up.
const INTERACT: i32 = 0;
const ATTACK: i32 = 1;
const INTERACT_AT: i32 = 2;

/// 26.1 split the interaction in two: an attack became its own packet, and the aimed-at point
/// moved into the interaction itself as a packed vector.
///
/// A client older than that sends an `interact_at` and then a plain `interact` for the same
/// gesture. Only the first carries the point, and acting on both would use the entity twice, so
/// the second is dropped.
///
/// Vanilla turns a spectator's attack into a spectate instead. Spectator mode is not tracked here
/// yet, so an attack stays an attack; see `docs/networking/known-gaps.md`.
pub fn interact<R: Read>(reader: &mut R, version: ProtocolVersion) -> Upgraded {
    if version >= NATIVE {
        return None;
    }
    Some((|| {
        let opts = NetDecodeOpts::None;
        let entity_id = VarInt::decode(reader, &opts)?;
        let action = VarInt::decode(reader, &opts)?;
        match action.0 {
            ATTACK => Ok(Upgrade::Into(super::upgraded_body(
                version,
                |body, opts| entity_id.encode(body, opts),
            )?)),
            INTERACT => Ok(Upgrade::Dropped),
            INTERACT_AT => {
                let x = f32::decode(reader, &opts)?;
                let y = f32::decode(reader, &opts)?;
                let z = f32::decode(reader, &opts)?;
                let hand = VarInt::decode(reader, &opts)?;
                let secondary = bool::decode(reader, &opts)?;
                Ok(Upgrade::Body(super::upgraded_body(
                    version,
                    |body, opts| {
                        entity_id.encode(body, opts)?;
                        hand.encode(body, opts)?;
                        LowPrecisionVec3::new(f64::from(x), f64::from(y), f64::from(z))
                            .encode(body, opts)?;
                        secondary.encode(body, opts)
                    },
                )?))
            }
            other => Err(NetDecodeError::ExternalError(
                format!("interaction {other} is not one this server knows").into(),
            )),
        }
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrumc_net_codec::encode::Framing;

    fn encoded_for(version: ProtocolVersion) -> Vec<u8> {
        let packet = UpdateTimePacket::overworld(1234, 6000, true);
        let mut buffer = Vec::new();
        packet
            .encode(&mut buffer, &NetEncodeOpts::new(Framing::None, version))
            .expect("encodes");
        buffer
    }

    /// The older form is a game time, a day time and a flag: eight, eight and one bytes, after the
    /// packet id.
    #[test]
    fn older_clients_get_the_flat_form() {
        let older = encoded_for(ProtocolVersion::V1_21_11);
        let id_length = older.len() - 17;
        assert_eq!(
            &older[id_length..id_length + 8],
            &1234i64.to_be_bytes(),
            "game time should come first"
        );
        assert_eq!(
            &older[id_length + 8..id_length + 16],
            &6000i64.to_be_bytes(),
            "the overworld clock's ticks become the day time"
        );
        assert_eq!(
            older[id_length + 16],
            1,
            "a running clock means time advances"
        );
    }

    /// 26.1 and later read the clock map, which is longer than the flat form.
    #[test]
    fn newer_clients_get_the_clock_map() {
        assert!(
            encoded_for(ProtocolVersion::V26_1).len()
                > encoded_for(ProtocolVersion::V1_21_11).len(),
            "the clock map should be longer than the three fields it replaced"
        );
    }

    /// A stopped clock has to reach the older client as "time does not advance".
    #[test]
    fn a_stopped_clock_stops_time() {
        let packet = UpdateTimePacket::overworld(1, 2, false);
        let mut buffer = Vec::new();
        packet
            .encode(
                &mut buffer,
                &NetEncodeOpts::new(Framing::None, ProtocolVersion::V1_21_11),
            )
            .expect("encodes");
        assert_eq!(*buffer.last().expect("not empty"), 0);
    }
}
