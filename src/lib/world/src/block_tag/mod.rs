//! How vanilla groups blocks that behave alike.
//!
//! Every door, every block a sapling grows on, everything a piston cannot push: these are the
//! groupings the game itself uses, and the ones behaviour registers against.
//!
//! The groupings are datapack json, not a table baked in here, so a pack that adds a block to
//! `#minecraft:logs` makes it a log for everything that asks. They are read once at startup and
//! again on a reload, and held here because the things that ask are scattered through the world
//! rather than gathered where a parameter could reach them.

use crate::block_state::{BlockId, BLOCKS};
use ferrumc_datapack::tag::{RawTags, TagId, TagRegistry};
use ferrumc_datapack::ResourceManager;
use std::sync::{Arc, LazyLock, RwLock};

/// Where a block tag file lives inside a pack.
pub const DIRECTORY: &str = "tags/block";

/// Reads the block tags out of a pack stack.
#[must_use]
pub fn load(manager: &ResourceManager) -> TagRegistry {
    RawTags::load(manager, DIRECTORY).build(BLOCKS.len(), |id| {
        BlockId::from_name(id.as_str()).map(|block| u32::from(block.index()))
    })
}

/// Falls back to the pack the server ships with, so anything that asks for a tag before the
/// datapacks are read still gets vanilla's answer rather than an empty one.
static TAGS: LazyLock<RwLock<Arc<TagRegistry>>> = LazyLock::new(|| {
    let built_in = ferrumc_datapack::vanilla_pack()
        .map(|pack| ResourceManager::new(vec![Arc::new(pack)]))
        .map(|manager| load(&manager))
        .unwrap_or_else(|e| {
            tracing::error!("could not read the built-in block tags: {e}");
            TagRegistry::new(BLOCKS.len())
        });
    RwLock::new(Arc::new(built_in))
});

/// Every block tag as the loaded packs declare them.
///
/// Hold on to this for the length of a piece of work rather than calling it per block: it takes
/// the lock and clones a handle each time.
#[must_use]
pub fn tags() -> Arc<TagRegistry> {
    TAGS.read()
        .expect("the block tags are never held across a panic")
        .clone()
}

/// Replaces the block tags, which is what loading or reloading datapacks does.
pub fn set(tags: Arc<TagRegistry>) {
    *TAGS
        .write()
        .expect("the block tags are never held across a panic") = tags;
}

/// Whether this tag holds the block.
///
/// Resolving the tag by name every time costs a hash; somewhere that asks per block per tick
/// should hold [`tags`] and its [`TagId`] instead.
#[must_use]
pub fn is_in(block: BlockId, tag: &str) -> bool {
    let tags = tags();
    tags.get_by_name(tag)
        .is_some_and(|tag| tags.contains(tag, u32::from(block.index())))
}

/// The blocks a tag holds, in the order it declares them.
#[must_use]
pub fn blocks(tag: &str) -> Vec<BlockId> {
    let tags = tags();
    let Some(tag) = tags.get_by_name(tag) else {
        return Vec::new();
    };
    tags.elements(tag)
        .iter()
        .filter_map(|&index| u16::try_from(index).ok().and_then(BlockId::from_index))
        .collect()
}

/// Whether the tag holds this block, for a caller that already has both to hand.
#[must_use]
pub fn tag_contains(tags: &TagRegistry, tag: TagId, block: BlockId) -> bool {
    tags.contains(tag, u32::from(block.index()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tags refer to other tags, and a list that still held the references would be missing most
    /// of its blocks: `doors` is mostly the `wooden_doors` it points at.
    #[test]
    fn references_between_tags_are_resolved() {
        let doors = blocks("minecraft:doors");
        let wooden = blocks("minecraft:wooden_doors");

        assert!(doors.len() > wooden.len());
        for block in wooden {
            assert!(
                doors.contains(&block),
                "{} is a wooden door but not a door",
                block.name()
            );
        }
        assert!(is_in(
            BlockId::from_name("minecraft:iron_door").expect("iron doors exist"),
            "minecraft:doors"
        ));
    }

    /// A tag holds what it says and nothing else.
    #[test]
    fn a_tag_holds_only_its_own() {
        let stone = BlockId::from_name("minecraft:stone").expect("stone exists");
        assert!(!is_in(stone, "minecraft:doors"));
        assert!(blocks("minecraft:not_a_tag").is_empty());
    }

    /// The whole of vanilla's block tags read, not a handful.
    #[test]
    fn the_built_in_pack_carries_the_vanilla_tags() {
        let tags = tags();
        assert!(
            tags.len() > 250,
            "vanilla has hundreds of block tags, found {}",
            tags.len()
        );
        assert!(is_in(
            BlockId::from_name("minecraft:oak_log").expect("oak logs exist"),
            "minecraft:logs"
        ));
    }

    /// A name without a namespace means the default one, as it does everywhere else.
    #[test]
    fn a_bare_name_is_a_minecraft_tag() {
        let oak = BlockId::from_name("minecraft:oak_log").expect("oak logs exist");
        assert!(is_in(oak, "logs"));
    }

    /// What datapack support is for: a pack that adds to a vanilla tag is answered by everything
    /// that asks about that tag, and what vanilla put there is still in it.
    #[test]
    fn a_pack_can_add_a_block_to_a_vanilla_tag() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = dir.path();
        let tag = root.join("data/minecraft/tags/block/logs.json");
        std::fs::create_dir_all(tag.parent().expect("a file has a parent"))
            .expect("a writable dir");
        std::fs::write(&tag, r#"{"values":["minecraft:sponge"]}"#).expect("a writable file");

        let stack = ResourceManager::new(vec![
            Arc::new(ferrumc_datapack::vanilla_pack().expect("the built-in pack opens")),
            Arc::new(
                ferrumc_datapack::DirPack::open("test", root.to_path_buf())
                    .expect("an openable pack"),
            ),
        ]);
        let tags = load(&stack);
        let logs = tags.get_by_name("minecraft:logs").expect("logs are tagged");

        let sponge = BlockId::from_name("minecraft:sponge").expect("sponge exists");
        let oak = BlockId::from_name("minecraft:oak_log").expect("oak logs exist");
        assert!(tag_contains(&tags, logs, sponge), "the pack's addition");
        assert!(tag_contains(&tags, logs, oak), "and what vanilla had");
    }

    /// A pack that says so drops what the packs below it declared rather than adding to it.
    #[test]
    fn a_pack_can_replace_a_vanilla_tag_outright() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = dir.path();
        let tag = root.join("data/minecraft/tags/block/logs.json");
        std::fs::create_dir_all(tag.parent().expect("a file has a parent"))
            .expect("a writable dir");
        std::fs::write(&tag, r#"{"replace":true,"values":["minecraft:sponge"]}"#)
            .expect("a writable file");

        let stack = ResourceManager::new(vec![
            Arc::new(ferrumc_datapack::vanilla_pack().expect("the built-in pack opens")),
            Arc::new(
                ferrumc_datapack::DirPack::open("test", root.to_path_buf())
                    .expect("an openable pack"),
            ),
        ]);
        let tags = load(&stack);
        let logs = tags.get_by_name("minecraft:logs").expect("logs are tagged");
        assert_eq!(tags.elements(logs).len(), 1);
    }
}
