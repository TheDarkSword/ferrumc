//! Everything 1.21.5 changed that a client on 1.21.4 or older does not read.

use super::Translated;
use crate::packets::outgoing::chunk_and_light_data::ChunkAndLightData;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_net_codec::version::ProtocolVersion;
use ferrumc_world::chunk::heightmap::{MOTION_BLOCKING, WORLD_SURFACE};
use std::io::Write;

/// The boundary this hop is about: everything below it predates 1.21.5's changes.
const NATIVE: ProtocolVersion = ProtocolVersion::V1_21_5;

/// Network NBT tag ids. The root of a network NBT value carries no name.
const TAG_END: u8 = 0x00;
const TAG_COMPOUND: u8 = 0x0A;
const TAG_LONG_ARRAY: u8 = 0x0C;

/// Chunk heightmaps became a list keyed by a numeric kind in 1.21.5. Before that they were an NBT
/// compound keyed by name, and a client of that era rejects the whole chunk when handed the newer
/// form — "expected root tag to be a CompoundTag" — so no terrain ever arrives.
pub fn level_chunk_with_light<W: Write>(
    packet: &ChunkAndLightData<'_>,
    writer: &mut W,
    opts: &NetEncodeOpts,
) -> Translated {
    if opts.version >= NATIVE {
        return None;
    }
    if let Err(err) = super::packet_id!(writer, opts, "play", "level_chunk_with_light") {
        return Some(Err(err));
    }
    Some((|| {
        packet.chunk_x.encode(writer, &opts.nested())?;
        packet.chunk_z.encode(writer, &opts.nested())?;

        write_heightmaps_as_nbt(packet, writer)?;
        packet.chunk_data.data.encode(writer, &opts.nested())?;

        packet.block_entities.encode(writer, &opts.nested())?;
        packet.light_data.encode(writer, &opts.nested())?;
        Ok(())
    })())
}

/// Writes the heightmaps as the unnamed root compound older clients expect.
fn write_heightmaps_as_nbt<W: Write>(
    packet: &ChunkAndLightData<'_>,
    writer: &mut W,
) -> Result<(), ferrumc_net_codec::encode::errors::NetEncodeError> {
    writer.write_all(&[TAG_COMPOUND])?;
    for entry in &packet.chunk_data.heightmaps.data {
        let name = match entry.heightmap.0 {
            WORLD_SURFACE => "WORLD_SURFACE",
            MOTION_BLOCKING => "MOTION_BLOCKING",
            // Kinds these versions do not know are simply left out.
            _ => continue,
        };

        writer.write_all(&[TAG_LONG_ARRAY])?;
        writer.write_all(&(name.len() as u16).to_be_bytes())?;
        writer.write_all(name.as_bytes())?;
        writer.write_all(&(entry.data.data.len() as i32).to_be_bytes())?;
        for value in &entry.data.data {
            writer.write_all(&value.to_be_bytes())?;
        }
    }
    writer.write_all(&[TAG_END])?;
    Ok(())
}
