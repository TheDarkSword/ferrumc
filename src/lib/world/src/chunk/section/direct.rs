use crate::chunk::remap::block_state_for;
use crate::chunk::section::paletted::PalettedSection;
use crate::chunk::section::uniform::UniformSection;
use crate::chunk::section::{AIR, CHUNK_SECTION_LENGTH};
use crate::chunk::BlockStateId;
use bitcode_derive::{Decode, Encode};
use deepsize::DeepSizeOf;
use ferrumc_net_codec::version::ProtocolVersion;

// Currently there are less block state ids than u16::MAX, so we can store ids as u16s to cut down on memory usage
/// Width of an entry in a section carrying global block state ids rather than a palette.
///
/// This is not free: it is `ceil(log2(block state count))`, and a client sizes its reads by it.
/// Every version this server speaks has between 26684 and 32366 block states, so all of them use
/// fifteen bits. Declaring sixteen packed the same four entries per long but told the client the
/// wrong width, and a translating proxy then read an empty palette out of the section.
pub const GLOBAL_PALETTE_BITS: u8 = 15;

type CompactBlockStateId = u16;

const AIR_COMPACT: CompactBlockStateId = AIR.raw() as CompactBlockStateId;

#[derive(Clone, DeepSizeOf, Encode, Decode)]
pub struct DirectSection(pub(crate) Box<[CompactBlockStateId]>, u16);

impl Default for DirectSection {
    fn default() -> Self {
        Self(
            vec![AIR_COMPACT; CHUNK_SECTION_LENGTH].into_boxed_slice(),
            0,
        )
    }
}

impl DirectSection {
    #[inline]
    pub fn set_block(&mut self, idx: usize, block: BlockStateId) {
        if self.0[idx] == AIR_COMPACT && block != AIR {
            self.1 += 1
        } else if self.0[idx] != AIR_COMPACT && block == AIR {
            self.1 -= 1
        }

        self.0[idx] = block.raw() as CompactBlockStateId;
    }

    #[inline]
    pub fn get_block(&self, idx: usize) -> BlockStateId {
        BlockStateId::new(self.0[idx] as _)
    }

    pub fn block_count(&self) -> u16 {
        self.1
    }

    /// Counts cells holding a fluid. Direct sections keep no palette, so every cell is inspected;
    /// they only appear once a section exceeds the palette limit, well off the common path.
    pub fn fluid_count(&self) -> u16 {
        self.0
            .iter()
            .filter(|&&id| crate::fluid::is_fluid(BlockStateId::new(id as _)))
            .count() as u16
    }

    /// Packs every block id into the chunk-packet "data array" layout: a stream of u64s with
    /// 16-bit entries, lowest index in the low bits, no spillover across longs.
    ///
    /// Entries are [`GLOBAL_PALETTE_BITS`] wide, so four fit in each long with the top bits
    /// unused; vanilla never lets an entry span two longs. We could in theory `bytemuck::cast_slice::<u16,
    /// u64>` the inner buffer, but that would assume a specific host endianness; the explicit
    /// shift below is portable and not on a hot path (chunk send, not per-tick).
    pub fn to_network_longs(&self, version: ProtocolVersion) -> Vec<u64> {
        const ENTRIES_PER_LONG: usize = 64 / GLOBAL_PALETTE_BITS as usize;
        const BITS_PER_ENTRY: usize = GLOBAL_PALETTE_BITS as usize;
        let mut out = vec![0u64; CHUNK_SECTION_LENGTH / ENTRIES_PER_LONG];
        for (i, &id) in self.0.iter().enumerate() {
            let long_idx = i / ENTRIES_PER_LONG;
            let bit_idx = (i % ENTRIES_PER_LONG) * BITS_PER_ENTRY;
            let id = block_state_for(u32::from(id), version);
            out[long_idx] |= u64::from(id) << bit_idx;
        }
        out
    }
}

impl From<&mut UniformSection> for DirectSection {
    fn from(s: &mut UniformSection) -> Self {
        Self(
            vec![s.get_block().raw() as CompactBlockStateId; CHUNK_SECTION_LENGTH]
                .into_boxed_slice(),
            if s.get_block() == AIR { 0 } else { 4096 },
        )
    }
}

impl From<&mut PalettedSection> for DirectSection {
    fn from(s: &mut PalettedSection) -> Self {
        let mut vec = vec![AIR_COMPACT; CHUNK_SECTION_LENGTH];
        let mut count = 0;

        for (block_idx, val) in vec.iter_mut().enumerate() {
            let block = s.get_block(block_idx);
            *val = s.get_block(block_idx).raw() as CompactBlockStateId;

            if block != AIR {
                count += 1
            }
        }

        Self(vec.into_boxed_slice(), count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip: every block id we put into a DirectSection must be recoverable from the packed
    /// network long array. This guards against the previous bug where the data_array was sent
    /// empty, causing every block to decode as 0 client-side (lava rendering as water etc.).
    #[test]
    fn to_network_longs_round_trips() {
        let mut section = DirectSection::default();
        // A handful of ids spread across the 4096 cells, including the boundaries between longs.
        let samples: &[(usize, u32)] = &[
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),      // first long
            (4, 100),    // second long, low entry
            (7, 32_767), // second long, the widest id fifteen bits can carry
            (4095, 7),   // last cell
        ];
        for &(idx, id) in samples {
            section.set_block(idx, BlockStateId::new(id));
        }

        let longs = section.to_network_longs(ProtocolVersion::CURRENT);
        assert_eq!(longs.len(), CHUNK_SECTION_LENGTH / 4);

        // Manually decode each long (four entries, lowest index in the low bits) and compare
        // against the section's stored ids.
        for (long_idx, _) in longs.iter().enumerate() {
            for entry in 0..4 {
                let block_idx = long_idx * 4 + entry;
                let width = u32::from(GLOBAL_PALETTE_BITS);
                let mask = (1u64 << width) - 1;
                let decoded = ((longs[long_idx] >> (entry as u32 * width)) & mask) as u32;
                assert_eq!(
                    decoded,
                    section.get_block(block_idx).raw(),
                    "mismatch at block_idx {}",
                    block_idx
                );
            }
        }
    }
}
