//! Datapacks: the layer everything the game defines in json is read through.
//!
//! A pack is a directory or a zip with a `pack.mcmeta` at its root and
//! `data/<namespace>/<registry>/<path>.json` underneath. Packs stack, and a later one overrides
//! an earlier one for the same location. The pack the server ships with is the bottom of that
//! stack and is carried inside the executable, so a player's pack in `datapacks/` overrides it
//! with no further plumbing.

pub mod error;
pub mod id;
pub mod manager;
pub mod meta;
pub mod pack;
pub mod repository;
pub mod tag;

pub use error::DatapackError;
pub use id::Identifier;
pub use manager::{FileToId, Resource, ResourceManager};
pub use meta::{PackCompatibility, PackFormat, PackMetadata};
pub use pack::{DirPack, PackResources, ZipPack};
pub use repository::{PackRepository, VANILLA_PACK_ID};
pub use tag::{RawTags, TagId, TagRegistry};

/// The pack the server ships with, built from the extracted vanilla data.
static VANILLA_PACK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/vanilla.zip"));

/// Opens the built-in pack. It is read exactly like a player's zip, so there is one reader rather
/// than a special case for the data the server was built with.
pub fn vanilla_pack() -> Result<ZipPack, DatapackError> {
    ZipPack::embedded(VANILLA_PACK_ID, VANILLA_PACK)
}

#[cfg(test)]
mod tests;
