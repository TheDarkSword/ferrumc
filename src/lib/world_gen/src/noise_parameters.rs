//! The octaves each named piece of noise is built from.
//!
//! Sixty-one of them, and every one is data rather than code: the packs say that `continents` is
//! octave -9 with nine amplitudes and `temperature` is octave -10 with five. Reading them means a
//! datapack that changes the shape of a world changes it here too, without a rebuild.

use crate::noise::Octaves;
use std::collections::BTreeMap;

/// Where the packs keep them.
const FROM: &str = "assets/extracted/26.2/data/minecraft/worldgen/noise";

/// One named set of octaves, as the packs write it.
#[derive(serde::Deserialize)]
struct AsWritten {
    #[serde(rename = "firstOctave")]
    first_octave: i32,
    amplitudes: Vec<f64>,
}

/// Every named piece of noise the packs define.
#[derive(Debug, Clone, Default)]
pub struct Parameters {
    named: BTreeMap<String, Octaves>,
}

impl Parameters {
    /// Reads them from the packs, relative to the given root.
    ///
    /// A name that cannot be read is left out rather than stopping the load: a pack with one bad
    /// file should cost that one piece of noise, not the world.
    #[must_use]
    pub fn load(root: &std::path::Path) -> Self {
        let mut named = BTreeMap::new();
        let Ok(entries) = std::fs::read_dir(root.join(FROM)) else {
            return Self { named };
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|kind| kind != "json") {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(written) = serde_json::from_str::<AsWritten>(&text) else {
                tracing::warn!("could not read the noise {name}");
                continue;
            };
            named.insert(
                name.to_string(),
                Octaves::new(written.first_octave, written.amplitudes),
            );
        }
        Self { named }
    }

    /// One by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Octaves> {
        self.named
            .get(name.strip_prefix("minecraft:").unwrap_or(name))
    }

    /// How many were read.
    #[must_use]
    pub fn len(&self) -> usize {
        self.named.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.named.is_empty()
    }

    /// Every name there is.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.named.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The repo root, from where this crate sits.
    ///
    /// Not `get_root_path`: that lives in `ferrumc-utils`, which reaches the world generator
    /// through the global state, so depending on it here would close a cycle.
    fn from_the_packs() -> Parameters {
        let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = crate_dir
            .ancestors()
            .nth(3)
            .expect("the crate sits three deep in the repository");
        Parameters::load(root)
    }

    #[test]
    fn every_piece_of_noise_the_packs_define_is_read() {
        let read = from_the_packs();
        assert_eq!(read.len(), 61);
    }

    /// The ones the terrain's shape rests on, with the octaves the packs give them.
    #[test]
    fn the_shape_of_the_world_reads_as_the_packs_write_it() {
        let read = from_the_packs();

        let continents = read.get("continentalness").expect("the packs define it");
        assert_eq!(continents.first, -9);
        assert_eq!(continents.amplitudes.len(), 9);

        let temperature = read.get("temperature").expect("the packs define it");
        assert_eq!(temperature.first, -10);
        assert_eq!(temperature.amplitudes, vec![1.5, 0.0, 1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn a_name_with_a_namespace_reads_the_same() {
        let read = from_the_packs();
        assert_eq!(read.get("minecraft:erosion"), read.get("erosion"));
    }

    #[test]
    fn a_name_the_packs_do_not_have_reads_as_nothing() {
        assert!(from_the_packs().get("no_such_noise").is_none());
    }
}
