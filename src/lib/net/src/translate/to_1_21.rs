//! Everything 1.21.2 changed that a client on 1.21 does not read.

use super::{Body, Translated, Upgraded};
use crate::packets::outgoing::entity_position_sync::TeleportEntityPacket;
use crate::packets::outgoing::set_container_content::SetContainerContent;
use crate::packets::outgoing::set_container_slot::SetContainerSlot;
use crate::packets::outgoing::set_player_inventory_slot::SetPlayerInventorySlot;
use crate::packets::outgoing::synchronise_vehicle_position::SynchroniseVehiclePosition;
use crate::packets::outgoing::synchronize_player_position::SynchronizePlayerPositionPacket;
use ferrumc_net_codec::decode::errors::NetDecodeError;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_net_codec::net_types::angle::NetAngle;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::version::ProtocolVersion;
use std::io::{Read, Write};

/// `ParticleStatus::All`, as a varint. 1.21 has no such setting and draws every particle.
const PARTICLE_STATUS_ALL: u8 = 0;

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

/// 1.21.2 gave the time its own daylight-cycle flag. 1.21 reads no such field and takes a negative
/// day time to mean the cycle is frozen, so the flag folds into the sign.
#[must_use]
pub fn set_time(day_time: i64, advancing: bool, version: ProtocolVersion) -> (i64, Option<bool>) {
    if version >= NATIVE {
        return (day_time, Some(advancing));
    }
    if advancing {
        (day_time, None)
    } else if day_time == 0 {
        // Zero has no negative, and a frozen dawn still has to read as frozen.
        (-1, None)
    } else {
        (-day_time, None)
    }
}

/// 1.21.2 rewrote the teleport: the id moved to the end, a velocity was added, and the relative
/// flags widened from a byte to an int. 1.21 reads the older shape.
///
/// The velocity has nowhere to go here. Vanilla clients on 1.21 are pushed by a separate motion
/// packet instead, which this does not send yet; see `docs/networking/known-gaps.md`.
pub fn player_position<W: Write>(
    packet: &SynchronizePlayerPositionPacket,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    if let Err(err) = super::packet_id!(writer, opts, "play", "player_position") {
        return Some(Err(err));
    }
    Some((|| {
        packet.x.encode(writer, &opts.nested())?;
        packet.y.encode(writer, &opts.nested())?;
        packet.z.encode(writer, &opts.nested())?;
        packet.yaw.encode(writer, &opts.nested())?;
        packet.pitch.encode(writer, &opts.nested())?;
        // Only the low eight bits were ever used, so the widened field truncates back cleanly.
        (packet.flags as u8).encode(writer, &opts.nested())?;
        packet.teleport_id.encode(writer, &opts.nested())?;
        Ok(())
    })())
}

/// 1.21.2 added a particle status to the client information, which a 1.21 client does not send.
/// It sits at the end, so the value that version behaved as is appended.
pub fn client_information<R: Read>(reader: &mut R, version: ProtocolVersion) -> Upgraded {
    if version >= NATIVE {
        return None;
    }
    Some((|| {
        let mut body = Vec::new();
        reader.read_to_end(&mut body)?;
        body.push(PARTICLE_STATUS_ALL);
        Ok(body)
    })())
}

/// The window a client on 1.21 addresses its own inventory by, which is where a
/// `set_player_inventory` lands once it becomes a plain slot update.
const PLAYER_INVENTORY_WINDOW: i8 = -2;

/// 1.21 has no inventory state to be out of step with when the server writes a slot directly.
const NO_STATE_ID: i32 = 0;

/// The teleport as 1.21 reads it: no velocity, rotation as two angle bytes rather than degrees,
/// and no relative-movement flags.
fn write_teleport<W: Write>(
    entity_id: &VarInt,
    (x, y, z): (f64, f64, f64),
    (yaw, pitch): (f32, f32),
    on_ground: bool,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Result<(), ferrumc_net_codec::encode::errors::NetEncodeError> {
    entity_id.encode(writer, &opts.nested())?;
    x.encode(writer, &opts.nested())?;
    y.encode(writer, &opts.nested())?;
    z.encode(writer, &opts.nested())?;
    NetAngle::from_degrees(f64::from(yaw)).encode(writer, &opts.nested())?;
    NetAngle::from_degrees(f64::from(pitch)).encode(writer, &opts.nested())?;
    on_ground.encode(writer, &opts.nested())?;
    Ok(())
}

/// 1.21.2 split the teleport in two, and what it added - a velocity and a set of relative-movement
/// flags - has no field in the older packet. 1.21 reads an `entity_position_sync` as the plain
/// `teleport_entity` it used to be.
pub fn entity_position_sync<W: Write>(
    packet: &TeleportEntityPacket,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    Some((|| {
        super::packet_id!(writer, opts, "play", "teleport_entity")?;
        write_teleport(
            &packet.entity_id,
            (packet.x, packet.y, packet.z),
            (packet.yaw, packet.pitch),
            packet.on_ground,
            writer,
            opts,
        )
    })())
}

/// The vehicle half of the same split, which reaches 1.21 as the same packet.
pub fn teleport_entity<W: Write>(
    packet: &SynchroniseVehiclePosition,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    Some((|| {
        super::packet_id!(writer, opts, "play", "teleport_entity")?;
        write_teleport(
            &packet.entity_id,
            (packet.x, packet.y, packet.z),
            (packet.yaw, packet.pitch),
            packet.on_ground,
            writer,
            opts,
        )
    })())
}

/// 1.21.2 gave the player inventory a packet of its own. Before it, writing a player's own slot was
/// an ordinary slot update against the inventory window.
pub fn set_player_inventory<W: Write>(
    packet: &SetPlayerInventorySlot,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    Some((|| {
        super::packet_id!(writer, opts, "play", "container_set_slot")?;
        PLAYER_INVENTORY_WINDOW.encode(writer, &opts.nested())?;
        VarInt::new(NO_STATE_ID).encode(writer, &opts.nested())?;
        (packet.slot_index.0 as i16).encode(writer, &opts.nested())?;
        packet.slot.encode(writer, &opts.nested())?;
        Ok(())
    })())
}

/// 1.21.2 widened the container id to a varint. 1.21 reads a signed byte here, which is what lets a
/// negative id mean the player's own inventory.
pub fn container_set_slot<W: Write>(
    packet: &SetContainerSlot,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    Some((|| {
        super::packet_id!(writer, opts, "play", "container_set_slot")?;
        (packet.window_id.0 as i8).encode(writer, &opts.nested())?;
        packet.state_id.encode(writer, &opts.nested())?;
        packet.slot_index.encode(writer, &opts.nested())?;
        packet.slot.encode(writer, &opts.nested())?;
        Ok(())
    })())
}

/// The same widening, on the packet that fills a whole container. This one reads its id unsigned,
/// since a container's contents are never written to the player's own inventory window.
pub fn container_set_content<W: Write>(
    packet: &SetContainerContent,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    Some((|| {
        super::packet_id!(writer, opts, "play", "container_set_content")?;
        (packet.window_id.0 as u8).encode(writer, &opts.nested())?;
        packet.state_id.encode(writer, &opts.nested())?;
        packet.slots.encode(writer, &opts.nested())?;
        packet.carried_item.encode(writer, &opts.nested())?;
        Ok(())
    })())
}

/// The world border was not what stopped the placement. 1.21 has no such field, and a client that
/// old cannot tell the server it hit one.
const WORLD_BORDER_NOT_HIT: u8 = 0;

/// 1.21.2 added a world border flag to the block placement, between the "inside block" flag and the
/// sequence number. Finding where it goes means walking the two varints ahead of it.
pub fn use_item_on<R: Read>(reader: &mut R, version: ProtocolVersion) -> Upgraded {
    if version >= NATIVE {
        return None;
    }
    Some((|| {
        let mut body = Vec::new();
        reader.read_to_end(&mut body)?;
        // Hand, then the block position, then the face, then three cursor floats and the flag.
        let mut at = super::varint_len(&body, 0)?;
        at += size_of::<u64>();
        at += super::varint_len(&body, at)?;
        at += 3 * size_of::<f32>() + 1;
        if at > body.len() {
            return Err(NetDecodeError::ExternalError(
                "a block placement ended before its cursor position".into(),
            ));
        }
        body.insert(at, WORLD_BORDER_NOT_HIT);
        Ok(body)
    })())
}

/// 1.21.2 widened the container id to a varint. A 1.21 client sends one signed byte, and a
/// negative id - the player's own inventory - is not the same varint as the byte it came in.
pub fn container_close<R: Read>(reader: &mut R, version: ProtocolVersion) -> Upgraded {
    if version >= NATIVE {
        return None;
    }
    Some((|| {
        let mut body = Vec::new();
        reader.read_to_end(&mut body)?;
        let Some(&id) = body.first() else {
            return Err(NetDecodeError::ExternalError(
                "a container close carried no container id".into(),
            ));
        };
        let mut out = Vec::new();
        VarInt::new(i32::from(id as i8))
            .encode(
                &mut out,
                &NetEncodeOpts::new(ferrumc_net_codec::encode::Framing::None, version),
            )
            .map_err(|err| NetDecodeError::ExternalError(Box::new(err)))?;
        Ok(out)
    })())
}

/// Bit positions of the movement flags 1.21.2 replaced the two axes with.
const FORWARD: u8 = 1;
const BACKWARD: u8 = 1 << 1;
const LEFT: u8 = 1 << 2;
const RIGHT: u8 = 1 << 3;
const JUMP: u8 = 1 << 4;
const SNEAK: u8 = 1 << 5;

/// What 1.21 sent instead: two signed axes and a flag byte holding jump and sneak.
const OLD_JUMP: u8 = 1;
const OLD_SNEAK: u8 = 1 << 1;
const OLD_INPUT_LENGTH: usize = 2 * size_of::<f32>() + 1;

/// 1.21.2 replaced the two movement axes with a bit per direction, and started sending the packet
/// outside vehicles as well.
///
/// The axes carried how hard a direction was held, which the flags cannot express; only the sign
/// survives. Sprinting has no bit in the older packet at all, and a 1.21 client reports it through
/// a player command instead.
pub fn player_input<R: Read>(reader: &mut R, version: ProtocolVersion) -> Upgraded {
    if version >= NATIVE {
        return None;
    }
    Some((|| {
        let mut body = Vec::new();
        reader.read_to_end(&mut body)?;
        if body.len() < OLD_INPUT_LENGTH {
            return Err(NetDecodeError::ExternalError(
                "a player input was shorter than its two axes and a flag".into(),
            ));
        }
        let axis =
            |at: usize| f32::from_be_bytes([body[at], body[at + 1], body[at + 2], body[at + 3]]);
        let sideways = axis(0);
        let forward = axis(size_of::<f32>());
        let old = body[2 * size_of::<f32>()];

        let mut flags = 0;
        if forward > 0.0 {
            flags |= FORWARD;
        } else if forward < 0.0 {
            flags |= BACKWARD;
        }
        if sideways > 0.0 {
            flags |= LEFT;
        } else if sideways < 0.0 {
            flags |= RIGHT;
        }
        if old & OLD_JUMP != 0 {
            flags |= JUMP;
        }
        if old & OLD_SNEAK != 0 {
            flags |= SNEAK;
        }
        Ok(vec![flags])
    })())
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

    /// The two axes only survive as a direction each, and jump and sneak move to new bits.
    #[test]
    fn the_movement_axes_become_flags() {
        let input = |sideways: f32, forward: f32, old: u8| {
            let mut body = sideways.to_be_bytes().to_vec();
            body.extend(forward.to_be_bytes());
            body.push(old);
            player_input(&mut body.as_slice(), ProtocolVersion::V1_21)
                .expect("1.21 is translated")
                .expect("translates")[0]
        };

        assert_eq!(input(0.0, 1.0, 0), FORWARD);
        assert_eq!(input(0.0, -1.0, 0), BACKWARD);
        assert_eq!(input(1.0, 0.0, 0), LEFT);
        assert_eq!(input(-1.0, 0.0, 0), RIGHT);
        assert_eq!(input(0.0, 0.0, OLD_JUMP), JUMP);
        assert_eq!(input(0.0, 0.0, OLD_SNEAK), SNEAK);
        // Half-pressed on a controller still only means forward.
        assert_eq!(input(0.0, 0.4, 0), FORWARD);
        assert_eq!(input(1.0, 1.0, OLD_JUMP), FORWARD | LEFT | JUMP);
    }

    /// A client from 1.21.2 on reads its own flags, so nothing is translated.
    #[test]
    fn newer_clients_send_the_flags_themselves() {
        assert!(player_input(&mut [0u8].as_slice(), ProtocolVersion::V1_21_2).is_none());
    }

    /// The flag goes between the "inside block" byte and the sequence, not at the end, and the
    /// varints ahead of it are not a fixed width.
    #[test]
    fn the_world_border_flag_lands_before_the_sequence() {
        // A hand and a face wide enough to need two bytes each, so a fixed offset would miss.
        let mut body = vec![0x80, 0x01];
        body.extend([0u8; 8]);
        body.extend([0x81, 0x01]);
        body.extend([1u8; 12]);
        body.push(1);
        body.push(42);

        let out = use_item_on(&mut body.as_slice(), ProtocolVersion::V1_21)
            .expect("1.21 is translated")
            .expect("translates");

        assert_eq!(out.len(), body.len() + 1);
        assert_eq!(out[out.len() - 2], WORLD_BORDER_NOT_HIT);
        assert_eq!(
            *out.last().expect("not empty"),
            42,
            "the sequence should survive"
        );
    }

    /// The player's own inventory is container -1, and a signed byte and a varint disagree about
    /// what that is.
    #[test]
    fn a_negative_container_id_survives_widening() {
        let out = container_close(&mut [0xFFu8].as_slice(), ProtocolVersion::V1_21)
            .expect("1.21 is translated")
            .expect("translates");
        assert_eq!(out, vec![0xFF, 0xFF, 0xFF, 0xFF, 0x0F], "-1 as a varint");
    }

    /// A frozen cycle reaches 1.21 as a negative day time, and dawn has to stay frozen rather than
    /// become a running midnight.
    #[test]
    fn a_frozen_cycle_folds_into_the_sign() {
        assert_eq!(set_time(6000, false, ProtocolVersion::V1_21), (-6000, None));
        assert_eq!(set_time(0, false, ProtocolVersion::V1_21), (-1, None));
        assert_eq!(set_time(6000, true, ProtocolVersion::V1_21), (6000, None));
    }

    /// 1.21.2 and up read the flag, so nothing is folded there.
    #[test]
    fn the_flag_survives_from_1_21_2() {
        assert_eq!(
            set_time(6000, false, ProtocolVersion::V1_21_2),
            (6000, Some(false))
        );
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
