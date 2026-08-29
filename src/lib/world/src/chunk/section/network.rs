use crate::chunk::palette::BlockPalette;
use crate::chunk::remap::block_state_for;
use crate::chunk::section::biome::{biome_palette_bits, BiomeData};
use crate::chunk::section::direct::DirectSection;
use crate::chunk::section::paletted::PalettedSection;
use crate::chunk::section::uniform::UniformSection;
use crate::chunk::section::{ChunkSection, ChunkSectionType, CHUNK_SECTION_LENGTH};
use ferrumc_macros::NetEncode;
use ferrumc_net_codec::encode::errors::NetEncodeError;
use ferrumc_net_codec::encode::{Framing, NetEncode, NetEncodeOpts};
use ferrumc_net_codec::net_types::net_array::NetworkArray;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::version::ProtocolVersion;

pub struct PalettedContainer<'section> {
    bits_per_entry: u8,
    palette: NetworkPalette,
    data_array: NetworkArray<'section, u64>,
}

/// The release that stopped prefixing a section's packed values with their length, because it can
/// be derived from the entry width. Older clients still read that length, and without it they take
/// the first long as a count and everything after the section is garbage.
const UNPREFIXED_VALUES_SINCE: ProtocolVersion = ProtocolVersion::V1_21_5;

impl NetEncode for PalettedContainer<'_> {
    fn encode<W: std::io::Write>(
        &self,
        writer: &mut W,
        opts: &NetEncodeOpts,
    ) -> Result<(), NetEncodeError> {
        self.bits_per_entry.encode(writer, &opts.nested())?;
        self.palette.encode(writer, &opts.nested())?;
        self.data_array.encode(writer, &self.values_opts(opts))
    }

    async fn encode_async<W: tokio::io::AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        opts: &NetEncodeOpts,
    ) -> Result<(), NetEncodeError> {
        self.bits_per_entry
            .encode_async(writer, &opts.nested())
            .await?;
        self.palette.encode_async(writer, &opts.nested()).await?;
        self.data_array
            .encode_async(writer, &self.values_opts(opts))
            .await
    }
}

impl PalettedContainer<'_> {
    fn values_opts(&self, opts: &NetEncodeOpts) -> NetEncodeOpts {
        if opts.version >= UNPREFIXED_VALUES_SINCE {
            opts.nested()
        } else {
            opts.framed(Framing::SizePrefixed)
        }
    }
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

/// The release that added a fluid count to every section. Sections are packed into an opaque byte
/// run before the chunk packet exists, so a packet-level translator cannot reach inside them; the
/// boundary has to be honoured here instead. See `ferrumc_net::translate`.
const FLUID_COUNT_SINCE: ProtocolVersion = ProtocolVersion::V26_1;

#[derive(NetEncode)]
pub struct NetworkSection<'section> {
    block_count: u16,
    /// Cells holding a fluid, from 26.1 on. The client reads it straight off the wire without
    /// recomputing, and uses it to decide whether entities moving through the section can be
    /// affected by fluid. Older clients have no such field and misread the section if sent one.
    fluid_count: Option<u16>,
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
        // Translating each entry can land two of them on the same block: the state space shrinks
        // going backwards, and in 1.21.4 over a thousand ids collapse onto stone alone. A palette
        // with duplicates is read by keying it on the id, which leaves fewer entries than the
        // packed data still indexes, so the palette is rebuilt and the indices rewritten.
        let translated: Vec<u32> = section
            .palette
            .palette_data()
            .into_iter()
            .map(|state| block_state_for(state.raw(), version))
            .collect();

        let mut unique: Vec<u32> = Vec::with_capacity(translated.len());
        let mut remapped_index: Vec<u8> = Vec::with_capacity(translated.len());
        for state in &translated {
            let index = unique.iter().position(|existing| existing == state);
            remapped_index.push(match index {
                Some(index) => index as u8,
                None => {
                    unique.push(*state);
                    (unique.len() - 1) as u8
                }
            });
        }

        // Everything in the section became the same block, which is what a single-valued section
        // says in one varint.
        if let [only] = unique[..] {
            return PalettedContainer {
                bits_per_entry: 0,
                palette: NetworkPalette::SingleValued {
                    value: VarInt(only as i32),
                },
                data_array: NetworkArray::new_owned(vec![]),
            };
        }

        // Four is the narrowest an indirect palette may be.
        let bits_per_entry = BlockPalette::bit_width_for_len(unique.len()).max(4);
        let unchanged = unique.len() == translated.len() && bits_per_entry == section.bit_width;

        let data_array: NetworkArray<u64> = if unchanged {
            NetworkArray::new_borrowed(&section.block_data)
        } else {
            let mut new_buffer = vec![
                0u64;
                (CHUNK_SECTION_LENGTH / (8 / bits_per_entry as usize))
                    / size_of::<u64>()
            ];

            for block in 0..CHUNK_SECTION_LENGTH {
                let old =
                    PalettedSection::unpack_value(&section.block_data, block, section.bit_width);
                PalettedSection::pack_value(
                    &mut new_buffer,
                    block,
                    bits_per_entry,
                    remapped_index[old as usize],
                );
            }

            NetworkArray::new_owned(new_buffer)
        };

        PalettedContainer {
            bits_per_entry,
            palette: NetworkPalette::Indirect {
                palette_length: VarInt(unique.len() as i32),
                palette_values: unique.into_iter().map(|s| VarInt(s as i32)).collect(),
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
            bits_per_entry: crate::chunk::section::direct::GLOBAL_PALETTE_BITS,
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

impl<'section> PalettedContainer<'section> {
    fn from_biomes(value: &'section BiomeData, version: ProtocolVersion) -> Self {
        match value {
            BiomeData::Uniform(data) => PalettedContainer {
                bits_per_entry: 0,
                palette: NetworkPalette::SingleValued {
                    value: VarInt(data.0 as _),
                },
                data_array: NetworkArray::new_owned(vec![]),
            },
            BiomeData::Mixed(data) => {
                // Packed explicitly rather than reinterpreted: the entries are narrower than a
                // byte, and casting the backing memory would also assume a host endianness.
                let bits = u32::from(biome_palette_bits(version));
                let per_long = 64 / bits as usize;
                let mut longs = vec![0u64; data.len().div_ceil(per_long)];
                for (index, biome) in data.iter().enumerate() {
                    longs[index / per_long] |=
                        u64::from(biome.0) << ((index % per_long) as u32 * bits);
                }
                PalettedContainer {
                    bits_per_entry: biome_palette_bits(version),
                    palette: NetworkPalette::Direct {},
                    data_array: NetworkArray::new_owned(longs),
                }
            }
        }
    }
}

impl<'section> NetworkSection<'section> {
    /// Serializes a section for a client speaking `version`. Block state ids are translated on the
    /// way out, because the same id means a different block in a different version.
    pub fn new(value: &'section ChunkSection, version: ProtocolVersion) -> Self {
        Self {
            block_count: value.block_count(),
            fluid_count: (version >= FLUID_COUNT_SINCE).then(|| value.fluid_count()),
            block_states: PalettedContainer::from_section(value, version),
            biomes: PalettedContainer::from_biomes(&value.biome, version),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::section::paletted::PalettedSection;
    use crate::chunk::BlockStateId;

    // 26.2 short grass and short dry grass both become 1.21.4's short grass. Translating a palette
    // holding both leaves two entries pointing at one block, which is what used to make a reader
    // run off the end of the palette.
    const SHORT_GRASS: u32 = 2248;
    const SHORT_DRY_GRASS: u32 = 2252;

    fn section_with(blocks: &[(usize, u32)]) -> PalettedSection {
        let mut section = PalettedSection::default();
        for &(index, state) in blocks {
            section.set_block(index, BlockStateId::new(state));
        }
        section
    }

    fn palette_of(container: &PalettedContainer<'_>) -> Vec<i32> {
        match &container.palette {
            NetworkPalette::SingleValued { value } => vec![value.0],
            NetworkPalette::Indirect { palette_values, .. } => {
                palette_values.iter().map(|v| v.0).collect()
            }
            NetworkPalette::Direct {} => vec![],
        }
    }

    /// Two blocks that become one must leave one palette entry, not two.
    #[test]
    fn a_translated_palette_has_no_duplicates() {
        let section = section_with(&[(0, SHORT_GRASS), (1, SHORT_DRY_GRASS)]);
        let container = PalettedContainer::from_paletted(&section, ProtocolVersion::V1_21_4);

        let palette = palette_of(&container);
        let mut unique = palette.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(
            palette.len(),
            unique.len(),
            "translated palette still holds duplicates: {palette:?}"
        );
    }

    /// The same section keeps both blocks apart for a version that has both.
    #[test]
    fn the_native_version_keeps_both_blocks() {
        let section = section_with(&[(0, SHORT_GRASS), (1, SHORT_DRY_GRASS)]);
        let container = PalettedContainer::from_paletted(&section, ProtocolVersion::CURRENT);

        let palette = palette_of(&container);
        assert!(
            palette.contains(&(SHORT_GRASS as i32)) && palette.contains(&(SHORT_DRY_GRASS as i32)),
            "both blocks should survive for a version that has them: {palette:?}"
        );
    }

    /// A palette that translates down to one block becomes a single-valued section rather than an
    /// indirect palette of length one. A paletted section always carries air at index zero, so the
    /// case that reaches this is a section holding nothing else.
    #[test]
    fn a_palette_of_one_block_becomes_single_valued() {
        let empty = PalettedSection::default();
        let container = PalettedContainer::from_paletted(&empty, ProtocolVersion::V1_21_4);
        assert!(
            matches!(container.palette, NetworkPalette::SingleValued { .. }),
            "a section holding only air is a single-valued section"
        );
        assert_eq!(
            container.bits_per_entry, 0,
            "a single value needs no data array"
        );
    }
}
