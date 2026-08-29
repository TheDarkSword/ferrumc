//! Everything 1.21.9 changed that a client on 1.21.7 or older does not read.

use super::Translated;
use crate::packets::outgoing::set_default_spawn_position::SetDefaultSpawnPositionPacket;
use crate::packets::outgoing::spawn_entity::SpawnEntityPacket;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_net_codec::version::ProtocolVersion;
use std::io::Write;

/// The boundary this hop is about: everything below it predates 1.21.9's changes.
const NATIVE: ProtocolVersion = ProtocolVersion::V1_21_9;

/// A velocity short counts eighth-thousandths of a block a tick.
const VELOCITY_UNITS: f64 = 8000.0;

/// 1.21.9 moved an entity's spawn movement ahead of its rotations and replaced the three velocity
/// shorts with a compressed vector. Older clients read the shorts, at the end, after the data
/// field.
pub fn add_entity<W: Write>(
    packet: &SpawnEntityPacket,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    if let Err(err) = super::packet_id!(writer, opts, "play", "add_entity") {
        return Some(Err(err));
    }
    Some((|| {
        packet.entity_id.encode(writer, &opts.nested())?;
        packet.entity_uuid.encode(writer, &opts.nested())?;
        packet.entity_type.encode(writer, &opts.nested())?;
        packet.x.encode(writer, &opts.nested())?;
        packet.y.encode(writer, &opts.nested())?;
        packet.z.encode(writer, &opts.nested())?;
        packet.pitch.encode(writer, &opts.nested())?;
        packet.yaw.encode(writer, &opts.nested())?;
        packet.head_yaw.encode(writer, &opts.nested())?;
        packet.data.encode(writer, &opts.nested())?;
        // The older form carries velocity as three shorts, in eight-thousandths of a block a tick.
        for axis in [packet.movement.x, packet.movement.y, packet.movement.z] {
            let ticks = (axis * VELOCITY_UNITS).clamp(f64::from(i16::MIN), f64::from(i16::MAX));
            (ticks as i16).encode(writer, &opts.nested())?;
        }
        Ok(())
    })())
}

/// 1.21.9 put the default spawn in a named dimension and gave it a pitch. Older clients read a
/// bare position and a yaw, and take the spawn to be in whatever dimension they are playing.
pub fn set_default_spawn_position<W: std::io::Write>(
    packet: &SetDefaultSpawnPositionPacket,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    Some((|| {
        super::packet_id!(writer, opts, "play", "set_default_spawn_position")?;
        packet.spawn_position.encode(writer, &opts.nested())?;
        packet.yaw.encode(writer, &opts.nested())?;
        Ok(())
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrumc_net_codec::encode::Framing;

    fn encoded_for(version: ProtocolVersion) -> Vec<u8> {
        let packet = SpawnEntityPacket::new(
            1,
            0,
            0,
            &ferrumc_core::transform::position::Position::new(0.0, 0.0, 0.0),
            &ferrumc_core::transform::rotation::Rotation::default(),
        );
        let mut buffer = Vec::new();
        packet
            .encode(&mut buffer, &NetEncodeOpts::new(Framing::None, version))
            .expect("encodes");
        buffer
    }

    /// The older form trades one movement byte for six velocity bytes.
    #[test]
    fn older_clients_get_velocity_shorts() {
        let native = encoded_for(ProtocolVersion::V1_21_9);
        let older = encoded_for(ProtocolVersion::V1_21_7);
        assert_eq!(
            older.len(),
            native.len() + 5,
            "the older form should be five bytes longer: three shorts instead of one byte"
        );
        assert_eq!(
            &older[older.len() - 6..],
            &[0u8; 6],
            "an entity spawned at rest has zero velocity"
        );
    }
}
