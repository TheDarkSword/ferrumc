use ferrumc_core::identity::player_identity::PlayerIdentity;
use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::length_prefixed_vec::LengthPrefixedVec;
use ferrumc_net_codec::net_types::var_int::VarInt;

#[derive(NetEncode)]
#[packet(packet_id = "remove_entities", state = "play")]
pub struct RemoveEntitiesPacket {
    pub entity_ids: LengthPrefixedVec<VarInt>,
}

impl RemoveEntitiesPacket {
    /// Tells a client to forget entities by the numbers it knows them by.
    ///
    /// Not everything a client is told to forget is a player, so this takes the numbers rather than
    /// the players holding them.
    #[must_use]
    pub fn of(entity_ids: &[i32]) -> Self {
        Self {
            entity_ids: LengthPrefixedVec::new(
                entity_ids.iter().copied().map(VarInt::new).collect(),
            ),
        }
    }

    pub fn from_entities<T>(entity_ids: T) -> Self
    where
        T: IntoIterator<Item = PlayerIdentity>,
    {
        let entity_ids: Vec<VarInt> = entity_ids
            .into_iter()
            .map(|entity| VarInt::new(entity.short_uuid))
            .collect();
        Self {
            entity_ids: LengthPrefixedVec::new(entity_ids),
        }
    }
}
