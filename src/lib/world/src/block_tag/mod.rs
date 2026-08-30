//! How vanilla groups blocks that behave alike.
//!
//! Every door, every block a sapling grows on, everything a piston cannot push: these are the
//! groupings the game itself uses, and the ones behaviour registers against. A name pattern would
//! do until the first block named unlike its family.

pub mod generated;

use crate::block_state::BlockId;
pub use generated::BlockTag;
use generated::TAGS;

impl BlockTag {
    /// Whether the tag holds this block.
    #[must_use]
    pub fn contains(&self, block: BlockId) -> bool {
        self.blocks.binary_search(&block.index()).is_ok()
    }

    /// Every block in the tag.
    pub fn blocks(&self) -> impl Iterator<Item = BlockId> + '_ {
        self.blocks
            .iter()
            .filter_map(|&index| BlockId::from_index(index))
    }
}

/// The tag of this name, `None` where the version has no such tag.
#[must_use]
pub fn tag(name: &str) -> Option<&'static BlockTag> {
    TAGS.binary_search_by_key(&name, |tag| tag.name)
        .ok()
        .map(|index| &TAGS[index])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tags refer to other tags, and a list that still held the references would be missing most
    /// of its blocks: `doors` is mostly the `wooden_doors` it points at.
    #[test]
    fn references_between_tags_are_resolved() {
        let doors = tag("minecraft:doors").expect("doors are tagged");
        let wooden = tag("minecraft:wooden_doors").expect("wooden doors are tagged");

        assert!(doors.blocks.len() > wooden.blocks.len());
        for block in wooden.blocks() {
            assert!(
                doors.contains(block),
                "{} is a wooden door but not a door",
                block.name()
            );
        }
        assert!(
            doors.contains(BlockId::from_name("minecraft:iron_door").expect("iron doors exist"))
        );
    }

    /// A tag holds what it says and nothing else.
    #[test]
    fn a_tag_holds_only_its_own() {
        let doors = tag("minecraft:doors").expect("doors are tagged");
        assert!(!doors.contains(BlockId::from_name("minecraft:stone").expect("stone exists")));
        assert!(tag("minecraft:not_a_tag").is_none());
    }
}
