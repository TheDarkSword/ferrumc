//! Telling a client what an entity's numbers are.
//!
//! A client draws several things from these rather than being told them outright: the attack
//! cooldown bar comes from `attack_speed`, how fast another player appears to move comes from
//! `movement_speed`, and how tall an entity is drawn comes from `scale`. Sending the base value and
//! the modifiers separately rather than the total is what lets a client show where a number came
//! from.
//!
//! The attribute registry has grown four times across the supported versions and was renamed once,
//! so which number an attribute travels as is looked up per version. An attribute a client has
//! never heard of is left out of the list rather than sent as another one.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::encode::errors::NetEncodeError;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::registry_remap::NetworkAttribute;
use std::io::Write;

/// What an entity's numbers are.
#[derive(NetEncode, Clone)]
#[packet(packet_id = "update_attributes", state = "play")]
pub struct UpdateAttributesPacket {
    pub entity_id: VarInt,
    values: AttributeList,
}

impl UpdateAttributesPacket {
    #[must_use]
    pub fn new(entity_id: i32, values: Vec<Snapshot>) -> Self {
        Self {
            entity_id: VarInt::new(entity_id),
            values: AttributeList(values),
        }
    }

    /// Whether it would say anything at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.0.is_empty()
    }
}

/// One attribute: what it was born as, and everything changing it.
#[derive(Clone)]
pub struct Snapshot {
    pub attribute: NetworkAttribute,
    pub base: f64,
    pub modifiers: Vec<WireModifier>,
}

/// One thing changing an attribute, as a client reads it.
#[derive(NetEncode, Clone)]
pub struct WireModifier {
    /// What the modifier is called, which is how a client tells two of them apart.
    pub name: String,
    pub amount: f64,
    pub operation: u8,
}

/// The list, written without whatever the reader has never heard of.
///
/// Counted after the dropping rather than before it: a length that does not match what follows is
/// how a client ends up reading the rest of the stream as attribute names.
#[derive(Clone)]
struct AttributeList(Vec<Snapshot>);

impl NetEncode for AttributeList {
    fn encode<W: Write>(&self, writer: &mut W, opts: &NetEncodeOpts) -> Result<(), NetEncodeError> {
        let nested = opts.nested();
        let known: Vec<&Snapshot> = self
            .0
            .iter()
            .filter(|snapshot| snapshot.attribute.known_to(opts.version))
            .collect();

        VarInt::new(i32::try_from(known.len()).unwrap_or(i32::MAX)).encode(writer, &nested)?;
        for snapshot in known {
            snapshot.attribute.encode(writer, &nested)?;
            snapshot.base.encode(writer, &nested)?;
            VarInt::new(i32::try_from(snapshot.modifiers.len()).unwrap_or(i32::MAX))
                .encode(writer, &nested)?;
            for modifier in &snapshot.modifiers {
                modifier.encode(writer, &nested)?;
            }
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use ferrumc_net_codec::encode::Framing;
    use ferrumc_net_codec::version::ProtocolVersion;

    fn bytes(packet: &UpdateAttributesPacket, version: ProtocolVersion) -> Vec<u8> {
        let mut out = Vec::new();
        packet
            .encode(&mut out, &NetEncodeOpts::new(Framing::None, version))
            .expect("a packet writes to a buffer");
        out
    }

    /// `max_health` sits at a different place in each of three eras.
    #[test]
    fn the_same_attribute_is_a_different_number_to_a_different_client() {
        let health = UpdateAttributesPacket::new(
            1,
            vec![Snapshot {
                attribute: NetworkAttribute(23),
                base: 20.0,
                modifiers: Vec::new(),
            }],
        );
        assert_ne!(
            bytes(&health, ProtocolVersion::V26_2),
            bytes(&health, ProtocolVersion::V1_21_7)
        );
    }

    /// An attribute a client has never heard of is dropped, and the count says so.
    #[test]
    fn what_a_client_does_not_know_is_left_out_and_not_counted() {
        // `bounciness` was added in 26.2 and nothing older has it.
        let bounciness = NetworkAttribute(9);
        assert!(!bounciness.known_to(ProtocolVersion::V1_21_7));

        let packet = UpdateAttributesPacket::new(
            1,
            vec![
                Snapshot {
                    attribute: bounciness,
                    base: 0.0,
                    modifiers: Vec::new(),
                },
                Snapshot {
                    attribute: NetworkAttribute(23),
                    base: 20.0,
                    modifiers: Vec::new(),
                },
            ],
        );

        // Written out in full, since what matters is the whole row rather than any one number:
        // the packet id, the entity, how many attributes follow, and then each of them as its
        // place in that client's own registry plus one, its base, and how many modifiers it has.
        assert_eq!(
            bytes(&packet, ProtocolVersion::V1_21_7),
            vec![
                124, // the packet id on 1.21.7
                1,   // the entity
                1,   // one attribute, because the other one does not exist here
                20,  // max_health, which sits at 19 on this version
                64, 52, 0, 0, 0, 0, 0, 0, // twenty, as a double
                0, // and nothing changing it
            ]
        );
        assert_eq!(
            bytes(&packet, ProtocolVersion::V26_2),
            vec![
                131, 1,  // the packet id on 26.2, which takes two bytes
                1,  // the entity
                2,  // both attributes
                10, // bounciness, which sits at 9 here and nowhere else
                0, 0, 0, 0, 0, 0, 0, 0,  // nothing, as a double
                0,  // and nothing changing it
                24, // max_health, which has moved to 23
                64, 52, 0, 0, 0, 0, 0, 0, //
                0,
            ]
        );
    }
}
