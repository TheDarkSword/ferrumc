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
