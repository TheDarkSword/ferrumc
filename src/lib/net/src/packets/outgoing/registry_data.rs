use bitcode::{Decode, Encode};
use ferrumc_macros::{build_registry_packets, packet, NetEncode};
use ferrumc_net_codec::net_types::length_prefixed_vec::LengthPrefixedVec;
use ferrumc_net_codec::net_types::prefixed_optional::PrefixedOptional;
use ferrumc_net_codec::version::ProtocolVersion;
use lazy_static::lazy_static;

#[derive(NetEncode)]
#[packet(packet_id = "registry_data", state = "configuration")]
pub struct RegistryDataPacket {
    pub registry_id: String,
    pub entries: LengthPrefixedVec<RegistryEntry>,
}

impl RegistryDataPacket {
    pub fn new(registry_id: String, entries: Vec<RegistryEntry>) -> Self {
        Self {
            registry_id,
            entries: LengthPrefixedVec::new(entries),
        }
    }
}

lazy_static! {
    /// The registry payload for each supported version, indexed by
    /// [`ProtocolVersion::index`]. Built once and reused; see [`registry_packets_for`].
    static ref REGISTRY_PACKETS: [Vec<RegistryDataPacket>; ProtocolVersion::ALL.len()] =
        build_registry_packets!().map(process_reg_packets);
}

/// The registries to send a client speaking `version`. Both the set and the contents differ
/// between releases, so a client must be sent its own version's payload.
pub fn registry_packets_for(version: ProtocolVersion) -> &'static [RegistryDataPacket] {
    &REGISTRY_PACKETS[version.index()]
}

fn process_reg_packets(payload: indexmap::IndexMap<String, Vec<u8>>) -> Vec<RegistryDataPacket> {
    payload
        .iter()
        .map(|(key, packets)| {
            let decoded: Vec<(String, Vec<u8>)> = bitcode::decode(packets).unwrap();
            RegistryDataPacket {
                registry_id: key.clone(),
                entries: LengthPrefixedVec::new(
                    decoded
                        .into_iter()
                        .map(|(id, data)| RegistryEntry {
                            id,
                            data: if data.is_empty() {
                                PrefixedOptional::None
                            } else {
                                PrefixedOptional::Some(data)
                            },
                        })
                        .collect(),
                ),
            }
        })
        .collect()
}

#[derive(NetEncode, Encode, Decode)]
pub struct RegistryEntry {
    pub id: String,
    pub data: PrefixedOptional<Vec<u8>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tag of one field of one entry, read back out of the payload the client is sent.
    fn tag_of(version: ProtocolVersion, registry: &str, entry: &str, field: &str) -> u8 {
        let packet = registry_packets_for(version)
            .iter()
            .find(|packet| packet.registry_id == registry)
            .unwrap_or_else(|| panic!("{registry} should be sent"));
        let data = packet
            .entries
            .data
            .iter()
            .find(|sent| sent.id == entry)
            .and_then(|sent| match &sent.data {
                PrefixedOptional::Some(data) => Some(data),
                PrefixedOptional::None => None,
            })
            .unwrap_or_else(|| panic!("{registry}/{entry} should carry data"));

        // Network NBT: a compound with no name, so the fields start straight after its tag byte.
        let mut at = 1;
        loop {
            let tag = data[at];
            assert_ne!(tag, 0, "{registry}/{entry} has no field {field}");
            at += 1;
            let name_len = usize::from(u16::from_be_bytes([data[at], data[at + 1]]));
            at += 2;
            let name = std::str::from_utf8(&data[at..at + name_len]).expect("a field name");
            if name == field {
                return tag;
            }
            at += name_len;
            at += skip(tag, &data[at..]);
        }
    }

    /// How many bytes a value of this tag takes.
    fn skip(tag: u8, data: &[u8]) -> usize {
        match tag {
            1 => 1,
            2 => 2,
            3 | 5 => 4,
            4 | 6 => 8,
            8 => 2 + usize::from(u16::from_be_bytes([data[0], data[1]])),
            7 => 4 + read_len(data),
            11 => 4 + read_len(data) * 4,
            12 => 4 + read_len(data) * 8,
            9 => {
                let element = data[0];
                let count = read_len(&data[1..]);
                let mut at = 5;
                for _ in 0..count {
                    at += skip(element, &data[at..]);
                }
                at
            }
            10 => {
                let mut at = 0;
                loop {
                    let inner = data[at];
                    at += 1;
                    if inner == 0 {
                        return at;
                    }
                    let name_len = usize::from(u16::from_be_bytes([data[at], data[at + 1]]));
                    at += 2 + name_len;
                    at += skip(inner, &data[at..]);
                }
            }
            other => panic!("unknown nbt tag {other}"),
        }
    }

    fn read_len(data: &[u8]) -> usize {
        usize::try_from(i32::from_be_bytes([data[0], data[1], data[2], data[3]]))
            .expect("a length is not negative")
    }

    const BYTE: u8 = 1;
    const INT: u8 = 3;
    const FLOAT: u8 = 5;

    /// The fields a strict client refused: a temperature is a float in the game's own codec, and
    /// sending it as a double is what made those clients drop the entry.
    #[test]
    fn a_field_carries_the_tag_the_game_gives_it() {
        for version in ProtocolVersion::ALL {
            assert_eq!(
                tag_of(version, "minecraft:worldgen/biome", "plains", "temperature"),
                FLOAT,
                "a biome's temperature on {version:?}"
            );
            assert_eq!(
                tag_of(version, "minecraft:worldgen/biome", "plains", "downfall"),
                FLOAT,
                "a biome's downfall on {version:?}"
            );
            assert_eq!(
                tag_of(
                    version,
                    "minecraft:worldgen/biome",
                    "plains",
                    "has_precipitation"
                ),
                BYTE,
                "a biome's precipitation on {version:?}"
            );
        }
    }

    /// The dimension type was the one registry with a hand-written schema; the measured table has
    /// to agree with what that got right.
    #[test]
    fn the_dimension_type_keeps_the_tags_it_had() {
        let version = ProtocolVersion::CURRENT;
        let of = |field| tag_of(version, "minecraft:dimension_type", "overworld", field);
        assert_eq!(of("ambient_light"), FLOAT);
        assert_eq!(of("height"), INT);
        assert_eq!(of("min_y"), INT);
        assert_eq!(of("logical_height"), INT);
        assert_eq!(of("has_skylight"), BYTE);
    }

    /// The enchantment registry is the one a strict client was measured refusing.
    #[test]
    fn an_enchantment_carries_its_own_tags() {
        let version = ProtocolVersion::CURRENT;
        let of = |field| tag_of(version, "minecraft:enchantment", "sharpness", field);
        assert_eq!(of("max_level"), INT);
        assert_eq!(of("weight"), INT);
        assert_eq!(of("anvil_cost"), INT);
    }
}
