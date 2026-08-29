use crate::pos::SectionBlockPos;
use bitcode_derive::{Decode, Encode};
use bytemuck::{Pod, Zeroable};
use deepsize::DeepSizeOf;

#[repr(transparent)]
#[derive(Copy, Clone, Encode, Decode, Default, PartialEq, DeepSizeOf, Pod, Zeroable)]
pub struct BiomeType(pub u8);

/// Width of an entry in a section carrying global biome ids rather than a palette, per supported
/// version, in the order of [`ProtocolVersion::ALL`].
///
/// Like the block palette this is `ceil(log2(count))` of what the version's registry holds, and a
/// strict reader sizes its reads by it rather than by what the packet declares. 1.21 and 1.21.2
/// ship 64 biomes; everything above them ships 65 or 66.
const BIOME_PALETTE_BITS: [u8; 10] = [6, 6, 7, 7, 7, 7, 7, 7, 7, 7];

/// The width a client speaking `version` expects a global biome id to occupy.
#[must_use]
pub fn biome_palette_bits(version: ferrumc_net_codec::version::ProtocolVersion) -> u8 {
    BIOME_PALETTE_BITS[version.index()]
}

#[derive(Clone, DeepSizeOf, Encode, Decode)]
pub enum BiomeData {
    Uniform(BiomeType),
    Mixed(Box<[BiomeType]>),
}

// Per-cell biome access has no caller yet: terrain generation currently fills a section with one
// biome. Phase 6's biome placement is what will use it.
#[expect(dead_code)]
impl BiomeData {
    pub fn new_uniform(value: BiomeType) -> Self {
        BiomeData::Uniform(value)
    }

    pub fn new_mixed() -> Self {
        BiomeData::Mixed(vec![BiomeType::default(); 64].into_boxed_slice())
    }

    pub fn fill_biome(&mut self, value: BiomeType) {
        *self = BiomeData::new_uniform(value);
    }

    pub fn set_biome(&mut self, value: BiomeType, pos: SectionBlockPos) {
        let idx = Self::get_idx(pos);

        match self {
            BiomeData::Uniform(data) => {
                if *data != value {
                    let mut new_data = vec![*data; 64].into_boxed_slice();
                    new_data[idx] = value;
                    *self = BiomeData::Mixed(new_data);
                }
            }
            BiomeData::Mixed(data) => data[idx] = value,
        }
    }

    pub fn get_biome(&self, pos: SectionBlockPos) -> BiomeType {
        let idx = Self::get_idx(pos);

        match self {
            BiomeData::Uniform(data) => *data,
            BiomeData::Mixed(data) => data[idx],
        }
    }

    fn get_idx(pos: SectionBlockPos) -> usize {
        let x = pos.x >> 2;
        let y = pos.y >> 2;
        let z = pos.z >> 2;

        ((y << 4) | (z << 2) | x) as usize
    }
}
