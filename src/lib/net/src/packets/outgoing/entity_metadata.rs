//! Telling a client what an entity looks like.
//!
//! The values themselves live in `ferrumc_entities::synced_data`; this is only how they reach a
//! client. Each one is written as the place it sits for that client, the number of the kind of
//! value that follows, and the value; a byte no field could ever sit at closes the row.
//!
//! Both the place and that number depend on the version being written to, and the version reaches
//! here through the encode options, so one packet is right for every client it is sent to.

use ferrumc_entities::entity_type::EntityType;
use ferrumc_entities::synced_data::{place_for, DataValue, SyncedData};
use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::encode::errors::NetEncodeError;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_net_codec::net_types::var_int::VarInt;
use std::io::Write;

/// No field sits here, so a client reading it knows the row has ended.
const END_OF_ROW: u8 = 0xFF;

/// What a client is told about an entity.
#[derive(NetEncode, Clone)]
#[packet(packet_id = "set_entity_data", state = "play")]
pub struct EntityMetadataPacket {
    entity_id: VarInt,
    values: MetadataRow,
}

impl EntityMetadataPacket {
    /// Everything there is to say about an entity, for a client that has just seen it.
    #[must_use]
    pub fn everything(entity_id: VarInt, data: &SyncedData) -> Self {
        Self {
            entity_id,
            values: MetadataRow::from(data, data.everything()),
        }
    }

    /// Only what has changed since the last time it was sent.
    ///
    /// Returns nothing when nothing has, since a row of no values is a packet worth not sending.
    #[must_use]
    pub fn changes(entity_id: VarInt, data: &SyncedData) -> Option<Self> {
        data.has_changes().then(|| Self {
            entity_id,
            values: MetadataRow::from(data, data.changes()),
        })
    }
}

/// One entity's values, each written where the client being written to keeps it.
#[derive(Clone)]
pub struct MetadataRow {
    /// The entity type these came off, which is what says where each of them sits.
    kind: EntityType,
    /// The values to write, each with the place it sits in the server's own terms.
    fields: Vec<(u8, DataValue)>,
}

impl MetadataRow {
    fn from<'a>(data: &SyncedData, values: impl Iterator<Item = (u8, &'a DataValue)>) -> Self {
        Self {
            kind: data.kind(),
            fields: values
                .map(|(index, value)| (index, value.clone()))
                .collect(),
        }
    }
}

impl NetEncode for MetadataRow {
    fn encode<W: Write>(&self, writer: &mut W, opts: &NetEncodeOpts) -> Result<(), NetEncodeError> {
        let nested = opts.nested();
        for (index, value) in &self.fields {
            // A version with no place for a field, or no such kind of value at all, is not told
            // about it: the numbers would land on whatever that version does keep there, and since
            // the kind says how many bytes follow, the rest of the row would be read at the wrong
            // offset.
            let Some((place, kind_id)) = place_for(self.kind, *index, opts.version) else {
                continue;
            };
            place.encode(writer, &nested)?;
            VarInt::new(i32::from(kind_id)).encode(writer, &nested)?;
            value.encode(writer, &nested)?;
        }
        END_OF_ROW.encode(writer, &nested)
    }

    async fn encode_async<W: tokio::io::AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        opts: &NetEncodeOpts,
    ) -> Result<(), NetEncodeError> {
        // A row is a few dozen bytes, so one buffer beats an await per value.
        let mut buffer = Vec::new();
        self.encode(&mut buffer, opts)?;
        buffer.encode_async(writer, &opts.nested()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrumc_entities::entity_type::EntityType;
    use ferrumc_entities::synced_data::{fields, Arm, EntityFlag, Pose};
    use ferrumc_net_codec::encode::Framing;
    use ferrumc_net_codec::version::ProtocolVersion;

    /// The row on its own, without the packet id or the length in front of it.
    fn row(data: &SyncedData, version: ProtocolVersion) -> Vec<u8> {
        let row = MetadataRow::from(data, data.changes());
        let mut bytes = Vec::new();
        row.encode(&mut bytes, &NetEncodeOpts::new(Framing::None, version))
            .expect("a row writes to a buffer");
        bytes
    }

    #[test]
    fn a_value_is_written_as_its_place_its_kind_and_itself() {
        let mut player = SyncedData::new(EntityType::Player);
        player.set_flag(EntityFlag::Crouching, true);
        player.set(fields::entity::POSE, Pose::Crouching);

        assert_eq!(
            row(&player, ProtocolVersion::V26_2),
            vec![
                0, 0, 0b10, // the shared flags byte, written as a byte, with crouching set
                6, 20, 5,    // the pose, written as a pose, crouching
                0xFF, // and nothing more
            ]
        );
    }

    #[test]
    fn a_row_a_client_can_read_ends_even_when_nothing_is_in_it() {
        let player = SyncedData::new(EntityType::Player);
        assert_eq!(row(&player, ProtocolVersion::V26_2), vec![0xFF]);
    }

    #[test]
    fn nothing_is_sent_when_nothing_changed() {
        let player = SyncedData::new(EntityType::Player);
        assert!(EntityMetadataPacket::changes(VarInt::new(1), &player).is_none());
    }

    #[test]
    fn a_value_carries_the_number_its_reader_knows_the_kind_by() {
        let mut player = SyncedData::new(EntityType::Player);
        player.set(fields::entity::POSE, Pose::Crouching);

        // Every version puts a pose at index six, and every version numbers the kind differently:
        // 1.21 has a compound tag where 26.2 has a particle, and everything after it shifts.
        assert_eq!(row(&player, ProtocolVersion::V26_2), vec![6, 20, 5, 0xFF]);
        assert_eq!(
            row(&player, ProtocolVersion::V1_21_11),
            vec![6, 20, 5, 0xFF]
        );
        assert_eq!(row(&player, ProtocolVersion::V1_21_7), vec![6, 21, 5, 0xFF]);
        assert_eq!(row(&player, ProtocolVersion::V1_21), vec![6, 21, 5, 0xFF]);
    }

    #[test]
    fn a_value_of_a_kind_a_client_has_never_heard_of_is_left_out() {
        // Which arm a player favours became a synced field in 26.1; before that there was no such
        // kind of value at all, and a number naming one would be read as something else.
        let mut player = SyncedData::new(EntityType::Player);
        player.set(fields::avatar::PLAYER_MAIN_HAND, Arm::Left);

        assert_eq!(row(&player, ProtocolVersion::V26_2), vec![15, 42, 0, 0xFF]);
        assert_eq!(row(&player, ProtocolVersion::V1_21), vec![0xFF]);
    }

    #[test]
    fn a_value_reaches_an_older_client_where_that_version_keeps_it() {
        let mut slime = SyncedData::new(EntityType::Slime);
        slime.set(fields::abstract_cube_mob::SIZE, 2);

        // 26.2 made a slime an ageable mob, which pushed its size two places down the row.
        assert_eq!(row(&slime, ProtocolVersion::V26_2), vec![18, 1, 2, 0xFF]);
        assert_eq!(row(&slime, ProtocolVersion::V26_1), vec![16, 1, 2, 0xFF]);
    }

    #[test]
    fn a_value_an_older_client_has_no_place_for_is_left_out() {
        let mut slime = SyncedData::new(EntityType::Slime);
        slime.set(fields::ageable_mob::BABY, true);

        assert_eq!(row(&slime, ProtocolVersion::V26_2), vec![16, 8, 1, 0xFF]);
        assert_eq!(
            row(&slime, ProtocolVersion::V26_1),
            vec![0xFF],
            "26.1 keeps something else at 16, and would read the byte as that"
        );
    }
}
