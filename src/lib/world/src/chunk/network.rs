use crate::chunk::heightmap::{Heightmaps, NetworkHeightmap};
use crate::chunk::section::network::NetworkSection;
use crate::chunk::Chunk;
use ferrumc_macros::NetEncode;
use ferrumc_net_codec::encode::errors::NetEncodeError;
use ferrumc_net_codec::encode::{Framing, NetEncode, NetEncodeOpts};
use ferrumc_net_codec::net_types::byte_array::ByteArray;
use ferrumc_net_codec::net_types::length_prefixed_vec::LengthPrefixedVec;
use ferrumc_net_codec::version::ProtocolVersion;
use std::io::Cursor;

#[derive(NetEncode)]
pub struct NetworkChunk {
    /// Public so a translator can write these in the form older clients expect: they became a list
    /// keyed by a numeric kind in 1.21.5, and were an NBT compound before that.
    pub heightmaps: LengthPrefixedVec<NetworkHeightmap>,
    /// The sections, already packed for the target version.
    pub data: ByteArray,
}

impl NetworkChunk {
    /// Serializes a chunk's sections for a client speaking `version`. Section layout is
    /// version-dependent, and the sections are packed into an opaque byte run here rather than
    /// during the packet's own encode, so the version has to be handed in.
    pub fn new(chunk: &Chunk, version: ProtocolVersion) -> Result<Self, NetEncodeError> {
        let heightmaps = Heightmaps::get_network_repr(&chunk.heightmaps);
        let mut data = Cursor::new(vec![]);
        let opts = NetEncodeOpts::new(Framing::None, version);

        for section in chunk.sections.iter() {
            NetworkSection::new(section, version).encode(&mut data, &opts)?;
        }

        Ok(Self {
            heightmaps,
            data: ByteArray::new(data.into_inner()),
        })
    }
}

impl Default for NetworkChunk {
    fn default() -> Self {
        Self {
            heightmaps: LengthPrefixedVec::default(),
            data: ByteArray::new(Vec::new()),
        }
    }
}
