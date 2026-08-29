use crate::chunk::remap::block_state_for;
use crate::chunk::section::biome::BiomeData;
use crate::chunk::section::direct::DirectSection;
use crate::chunk::section::paletted::PalettedSection;
use crate::chunk::section::uniform::UniformSection;
use crate::chunk::section::{ChunkSection, ChunkSectionType, CHUNK_SECTION_LENGTH};
use ferrumc_macros::NetEncode;
use ferrumc_net_codec::net_types::net_array::NetworkArray;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::version::ProtocolVersion;

#[derive(NetEncode)]
pub struct PalettedContainer<'section> {
    bits_per_entry: u8,
    palette: NetworkPalette,
    data_array: NetworkArray<'section, u64>,
}

#[derive(NetEncode)]
pub enum NetworkPalette {
    SingleValued {
        value: VarInt,
    },
    Indirect {
        palette_length: VarInt,
        palette_values: Vec<VarInt>,
    },
    Direct {
        // No values
    },
}

#[derive(NetEncode)]
pub struct NetworkSection<'section> {
    block_count: u16,
    /// Cells holding a fluid. The client reads it straight off the wire without recomputing, and
    /// uses it to decide whether entities moving through the section can be affected by fluid.
    fluid_count: u16,
    block_states: PalettedContainer<'section>,
    biomes: PalettedContainer<'section>,
}

impl<'section> PalettedContainer<'section> {
    fn from_uniform(section: &'section UniformSection, version: ProtocolVersion) -> Self {
        PalettedContainer {
            bits_per_entry: 0,
            palette: NetworkPalette::SingleValued {
                value: VarInt(block_state_for(section.get_block().raw() as _, version) as _),
            },
            data_array: NetworkArray::new_owned(vec![]),
        }
    }
}

impl<'section> PalettedContainer<'section> {
    fn from_paletted(section: &'section PalettedSection, version: ProtocolVersion) -> Self {
        let bits_per_entry = section.bit_width.max(4); // Minecraft supports lowest bit width of 4 for indirect palettes
        let data_array: NetworkArray<u64> = if bits_per_entry != section.bit_width {
            let mut new_buffer = vec![
                0u64;
                (CHUNK_SECTION_LENGTH / (8 / bits_per_entry as usize))
                    / size_of::<u64>()
            ];

            for block in 0..CHUNK_SECTION_LENGTH {
                PalettedSection::pack_value(
                    &mut new_buffer,
                    block,
                    bits_per_entry,
                    PalettedSection::unpack_value(&section.block_data, block, section.bit_width),
                );
            }

            NetworkArray::new_owned(new_buffer)
        } else {
            NetworkArray::new_borrowed(&section.block_data)
        };

        PalettedContainer {
            bits_per_entry,
            palette: NetworkPalette::Indirect {
                palette_length: VarInt(section.palette.len() as _),
                palette_values: section
                    .palette
                    .palette_data()
                    .into_iter()
                    .map(|v| VarInt(block_state_for(v.raw() as _, version) as _))
                    .collect(),
            },
            data_array,
        }
    }
}

impl<'section> PalettedContainer<'section> {
    fn from_direct(section: &'section DirectSection, version: ProtocolVersion) -> Self {
        // Direct sections carry one global-palette id per block. Without a real data_array the
        // client decoded every block as 0, which is what made e.g. lava sections render with
        // water's texture on the initial chunk send.
        PalettedContainer {
            bits_per_entry: 16,
            palette: NetworkPalette::Direct {},
            data_array: NetworkArray::new_owned(section.to_network_longs(version)),
        }
    }
}

impl<'section> PalettedContainer<'section> {
    fn from_section(value: &'section ChunkSection, version: ProtocolVersion) -> Self {
        match &value.inner {
            ChunkSectionType::Uniform(data) => Self::from_uniform(data, version),
            ChunkSectionType::Paletted(data) => Self::from_paletted(data, version),
            ChunkSectionType::Direct(data) => Self::from_direct(data, version),
        }
    }
}

impl<'section> From<&'section BiomeData> for PalettedContainer<'section> {
    fn from(value: &'section BiomeData) -> Self {
        match value {
            BiomeData::Uniform(data) => PalettedContainer {
                bits_per_entry: 0,
                palette: NetworkPalette::SingleValued {
                    value: VarInt(data.0 as _),
                },
                data_array: NetworkArray::new_owned(vec![]),
            },
            BiomeData::Mixed(data) => PalettedContainer {
                bits_per_entry: 8,
                palette: NetworkPalette::Direct {},
                data_array: NetworkArray::new_borrowed(bytemuck::cast_slice(data)),
            },
        }
    }
}

impl<'section> NetworkSection<'section> {
    /// Serializes a section for a client speaking `version`. Block state ids are translated on the
    /// way out, because the same id means a different block in a different version.
    pub fn new(value: &'section ChunkSection, version: ProtocolVersion) -> Self {
        Self {
            block_count: value.block_count(),
            fluid_count: value.fluid_count(),
            block_states: PalettedContainer::from_section(value, version),
            biomes: PalettedContainer::from(&value.biome),
        }
    }
}
