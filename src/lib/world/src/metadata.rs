//! What a world remembers about itself between runs.

use crate::errors::WorldError;
use crate::World;

/// Table holding the handful of values that describe a world rather than its contents.
const TABLE: &str = "world_metadata";
/// Key the generation seed is stored under.
const SEED_KEY: u128 = 0;

impl World {
    /// The seed this world's terrain was generated with, or `None` for a world that predates the
    /// seed being recorded or has never been generated.
    pub fn seed(&self) -> Result<Option<u64>, WorldError> {
        if !self.storage_backend.table_exists(TABLE.to_string())? {
            return Ok(None);
        }
        let stored = self.storage_backend.get(TABLE.to_string(), SEED_KEY)?;
        Ok(stored
            .and_then(|bytes| bytes.try_into().ok())
            .map(u64::from_le_bytes))
    }

    /// Records the seed a new world was generated with, so later runs shape the terrain they have
    /// not reached yet the same way as the terrain they already wrote.
    pub fn set_seed(&self, seed: u64) -> Result<(), WorldError> {
        if !self.storage_backend.table_exists(TABLE.to_string())? {
            self.storage_backend.create_table(TABLE.to_string())?;
        }
        self.storage_backend
            .upsert(TABLE.to_string(), SEED_KEY, seed.to_le_bytes().to_vec())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_world() -> (World, tempfile::TempDir) {
        let directory = tempfile::tempdir().expect("temp dir");
        let world = World::new(directory.path());
        (world, directory)
    }

    /// A world that has never been generated has no seed to honour.
    #[test]
    fn a_fresh_world_records_nothing() {
        let (world, _dir) = temp_world();
        assert_eq!(world.seed().expect("reads"), None);
    }

    /// The seed survives, so a later run shapes new terrain the way the written terrain was
    /// shaped rather than leaving a seam where generation resumed.
    #[test]
    fn a_recorded_seed_is_read_back() {
        let (world, _dir) = temp_world();
        world.set_seed(0x0123_4567_89AB_CDEF).expect("writes");
        assert_eq!(world.seed().expect("reads"), Some(0x0123_4567_89AB_CDEF));
    }

    /// Recording again replaces the value rather than failing or appending.
    #[test]
    fn recording_again_replaces_it() {
        let (world, _dir) = temp_world();
        world.set_seed(1).expect("writes");
        world.set_seed(2).expect("writes again");
        assert_eq!(world.seed().expect("reads"), Some(2));
    }
}
