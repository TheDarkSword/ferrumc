//! Telling a client that something was hurt, and by what.
//!
//! This is what produces the red flash and the tilt away from whatever did it. The kind of damage
//! is sent as a place in the client's own damage type registry, and that registry has grown four
//! times across the supported versions — so the number is looked up per version, the same way the
//! entity metadata row is, rather than assumed to be the newest one.

use ferrumc_data::generated::damage_types::DamageType;
use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::encode::errors::NetEncodeError;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_net_codec::net_types::prefixed_optional::PrefixedOptional;
use ferrumc_net_codec::net_types::var_int::VarInt;
use std::io::Write;

/// What is sent in place of an entity id when there is no entity behind the damage.
///
/// The wire carries the id plus one, so nothing at all is zero.
const NOBODY: i32 = -1;

/// Something was hurt, and this is what did it.
#[derive(NetEncode, Clone)]
#[packet(packet_id = "damage_event", state = "play")]
pub struct DamageEventPacket {
    entity_id: VarInt,
    kind: WireDamageType,
    /// Whoever is to blame, which for a fired arrow is the archer.
    cause: OptionalEntityId,
    /// Whatever actually touched it, which for a fired arrow is the arrow.
    direct: OptionalEntityId,
    /// Where the blow came from, where it came from a place rather than a thing.
    ///
    /// A plain `Option` writes nothing at all when it is empty, and the client is reading a
    /// boolean here whether or not there is a position behind it.
    from: PrefixedOptional<Source>,
}

impl DamageEventPacket {
    /// A blow from the world itself: falling, drowning, burning, the void.
    #[must_use]
    pub const fn from_the_world(entity_id: i32, kind: DamageType) -> Self {
        Self {
            entity_id: VarInt::new(entity_id),
            kind: WireDamageType(kind),
            cause: OptionalEntityId(NOBODY),
            direct: OptionalEntityId(NOBODY),
            from: PrefixedOptional::None,
        }
    }

    /// A blow from something, which is to blame for it and may not be what touched it.
    #[must_use]
    pub const fn from_an_entity(entity_id: i32, kind: DamageType, cause: i32, direct: i32) -> Self {
        Self {
            entity_id: VarInt::new(entity_id),
            kind: WireDamageType(kind),
            cause: OptionalEntityId(cause),
            direct: OptionalEntityId(direct),
            from: PrefixedOptional::None,
        }
    }
}

/// A kind of damage, written as the place the client being written to keeps it.
#[derive(Clone, Copy)]
struct WireDamageType(DamageType);

impl NetEncode for WireDamageType {
    fn encode<W: Write>(&self, writer: &mut W, opts: &NetEncodeOpts) -> Result<(), NetEncodeError> {
        // A registry entry is written as its place plus one, since zero means the entry follows
        // inline. A version that has never heard of this kind is told the most general one there
        // is, which every version has and which no client will misread as something specific.
        let place = self
            .0
            .wire_id(opts.version)
            .or_else(|| DamageType::Generic.wire_id(opts.version))
            .unwrap_or(0);
        VarInt::new(place + 1).encode(writer, &opts.nested())
    }

    async fn encode_async<W: tokio::io::AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        opts: &NetEncodeOpts,
    ) -> Result<(), NetEncodeError> {
        let mut buffer = Vec::new();
        self.encode(&mut buffer, opts)?;
        buffer.encode_async(writer, &opts.nested()).await
    }
}

/// An entity id, or nothing, written as the id plus one.
#[derive(Clone, Copy)]
struct OptionalEntityId(i32);

impl NetEncode for OptionalEntityId {
    fn encode<W: Write>(&self, writer: &mut W, opts: &NetEncodeOpts) -> Result<(), NetEncodeError> {
        VarInt::new(self.0 + 1).encode(writer, &opts.nested())
    }

    async fn encode_async<W: tokio::io::AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        opts: &NetEncodeOpts,
    ) -> Result<(), NetEncodeError> {
        VarInt::new(self.0 + 1)
            .encode_async(writer, &opts.nested())
            .await
    }
}

/// Where a blow came from, when it came from a place.
#[derive(NetEncode, Clone, Copy)]
pub struct Source {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrumc_net_codec::encode::Framing;
    use ferrumc_net_codec::version::ProtocolVersion;

    fn bytes(packet: &DamageEventPacket, version: ProtocolVersion) -> Vec<u8> {
        let mut out = Vec::new();
        packet
            .encode(&mut out, &NetEncodeOpts::new(Framing::None, version))
            .expect("a packet writes to a buffer");
        out
    }

    #[test]
    fn the_same_blow_is_a_different_number_to_a_different_client() {
        let hurt = DamageEventPacket::from_the_world(7, DamageType::SonicBoom);
        assert_ne!(
            bytes(&hurt, ProtocolVersion::V26_2),
            bytes(&hurt, ProtocolVersion::V1_21),
            "the registry grew in between, so the place moved"
        );
    }

    #[test]
    fn a_kind_a_client_never_heard_of_falls_back_to_something_it_has() {
        // Sending a place past the end of a client's registry is how a client is disconnected.
        let new = DamageEventPacket::from_the_world(7, DamageType::Spear);
        let generic = DamageEventPacket::from_the_world(7, DamageType::Generic);
        assert_eq!(
            bytes(&new, ProtocolVersion::V1_21),
            bytes(&generic, ProtocolVersion::V1_21)
        );
        assert_ne!(
            bytes(&new, ProtocolVersion::V26_2),
            bytes(&generic, ProtocolVersion::V26_2),
            "a client that does have it is told the truth"
        );
    }

    #[test]
    fn nothing_to_blame_is_written_as_zero() {
        let hurt = bytes(
            &DamageEventPacket::from_the_world(1, DamageType::Fall),
            ProtocolVersion::V26_2,
        );
        // entity id, kind, the two ids as nothing, and the boolean saying there is no position.
        assert_eq!(hurt[hurt.len() - 3..], [0, 0, 0]);
    }
}
