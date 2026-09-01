//! Keeping entities between one run of the server and the next.
//!
//! An entity belongs to the chunk it stands in, and is written and read with that chunk rather
//! than one at a time: a chunk nobody is near costs nothing, and a chunk that comes back brings its
//! mobs with it.
//!
//! Vanilla keeps these in a region file of their own rather than beside the blocks, so that adding
//! to one cannot cost the other. The same separation is kept here — a table of its own, keyed the
//! same way chunks are.

use crate::errors::WorldError;
use crate::pos::ChunkPos;
use crate::World;

/// Where entities are kept, apart from the blocks they stand on.
pub const ENTITIES: &str = "entities";

/// The layout of a saved entity. A stored list written by another layout is dropped rather than
/// read wrongly, which costs the mobs in a chunk and nothing else.
const ENTITY_FORMAT_VERSION: u32 = 1;

/// One entity, as it is written to disk.
///
/// What an entity is beyond this — what it is holding, how it is feeling — belongs to the mob and
/// is not written yet, so one comes back as an ordinary one of its kind standing where it stood.
#[derive(Debug, Clone, PartialEq, bitcode::Encode, bitcode::Decode)]
pub struct SavedEntity {
    /// The registry's own number for the kind, which is what the type enum carries.
    pub kind: u16,
    pub uuid: u128,
    pub position: [f64; 3],
    /// Yaw then pitch, as the wire orders them.
    pub rotation: [f32; 2],
    pub velocity: [f32; 3],
    pub on_ground: bool,
}

impl World {
    /// Writes the entities standing in a chunk, replacing whatever was there.
    ///
    /// An empty list removes the entry rather than storing nothing, so a chunk everything has left
    /// costs no space.
    pub fn save_entities(
        &self,
        pos: ChunkPos,
        dimension: &str,
        entities: &[SavedEntity],
    ) -> Result<(), WorldError> {
        let key = crate::db_functions::create_key(dimension, pos);
        if entities.is_empty() {
            if self.storage_backend.table_exists(ENTITIES.to_string())? {
                self.storage_backend.delete(ENTITIES.to_string(), key)?;
            }
            return Ok(());
        }

        if !self.storage_backend.table_exists(ENTITIES.to_string())? {
            self.storage_backend.create_table(ENTITIES.to_string())?;
        }
        let mut bytes = ENTITY_FORMAT_VERSION.to_le_bytes().to_vec();
        bytes.extend(bitcode::encode(entities));
        self.storage_backend
            .upsert(ENTITIES.to_string(), key, bytes)?;
        Ok(())
    }

    /// The entities standing in a chunk, or none where it has never held any.
    pub fn load_entities(
        &self,
        pos: ChunkPos,
        dimension: &str,
    ) -> Result<Vec<SavedEntity>, WorldError> {
        if !self.storage_backend.table_exists(ENTITIES.to_string())? {
            return Ok(Vec::new());
        }
        let key = crate::db_functions::create_key(dimension, pos);
        let Some(bytes) = self.storage_backend.get(ENTITIES.to_string(), key)? else {
            return Ok(Vec::new());
        };
        let Some((stamp, payload)) = bytes.split_at_checked(size_of::<u32>()) else {
            return Ok(Vec::new());
        };
        let version = u32::from_le_bytes([stamp[0], stamp[1], stamp[2], stamp[3]]);
        if version != ENTITY_FORMAT_VERSION {
            return Ok(Vec::new());
        }
        Ok(bitcode::decode(payload).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_zombie() -> SavedEntity {
        SavedEntity {
            kind: 130,
            uuid: 42,
            position: [8.5, 64.0, 8.5],
            rotation: [90.0, 0.0],
            velocity: [0.0, -0.08, 0.0],
            on_ground: true,
        }
    }

    #[test]
    fn a_chunks_entities_read_back_as_themselves() {
        let temp = tempfile::tempdir().expect("a directory to write into");
        let world = World::new(temp.path());
        let at = ChunkPos::new(3, -7);

        world
            .save_entities(at, "overworld", &[a_zombie()])
            .expect("writing entities");
        assert_eq!(
            world.load_entities(at, "overworld").expect("reading them"),
            vec![a_zombie()]
        );
    }

    #[test]
    fn a_chunk_that_never_held_anything_holds_nothing() {
        let temp = tempfile::tempdir().expect("a directory to write into");
        let world = World::new(temp.path());
        assert!(world
            .load_entities(ChunkPos::new(0, 0), "overworld")
            .expect("reading an empty chunk")
            .is_empty());
    }

    #[test]
    fn a_chunk_everything_left_stops_costing_anything() {
        let temp = tempfile::tempdir().expect("a directory to write into");
        let world = World::new(temp.path());
        let at = ChunkPos::new(1, 1);

        world
            .save_entities(at, "overworld", &[a_zombie()])
            .expect("writing entities");
        world
            .save_entities(at, "overworld", &[])
            .expect("writing none");
        assert!(world
            .load_entities(at, "overworld")
            .expect("reading it back")
            .is_empty());
    }

    #[test]
    fn two_dimensions_do_not_share_a_chunk() {
        let temp = tempfile::tempdir().expect("a directory to write into");
        let world = World::new(temp.path());
        let at = ChunkPos::new(0, 0);

        world
            .save_entities(at, "overworld", &[a_zombie()])
            .expect("writing entities");
        assert!(world
            .load_entities(at, "the_nether")
            .expect("reading the other one")
            .is_empty());
    }
}
