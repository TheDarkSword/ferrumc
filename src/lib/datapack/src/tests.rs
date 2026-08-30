//! What the datapack layer has to do: the built-in pack loads, a player's pack overrides it, and
//! a reload picks up what changed on disk.

use crate::id::Identifier;
use crate::meta::CURRENT_PACK_FORMAT;
use crate::repository::{PackRepository, VANILLA_PACK_ID};
use std::fs;
use std::path::Path;

const LOGS: &str = "minecraft:tags/block/logs.json";

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("a file has a parent")).expect("a writable dir");
    fs::write(path, contents).expect("a writable file");
}

/// A pack directory declaring itself built for the running game.
fn write_pack(root: &Path, name: &str) {
    write(
        &root.join(name).join("pack.mcmeta"),
        &format!(
            r#"{{"pack":{{"description":"test","min_format":[{},{}],"max_format":[{}]}}}}"#,
            CURRENT_PACK_FORMAT.major, CURRENT_PACK_FORMAT.minor, CURRENT_PACK_FORMAT.major
        ),
    );
}

#[test]
fn the_built_in_pack_is_the_bottom_of_the_stack() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let repository =
        PackRepository::discover(dir.path().join("datapacks")).expect("the built-in pack opens");

    assert_eq!(repository.selected(), [VANILLA_PACK_ID]);
    let vanilla = repository.get(VANILLA_PACK_ID).expect("the built-in pack");
    assert!(vanilla.compatibility.is_compatible());

    let manager = repository.open();
    let logs = Identifier::parse(LOGS).expect("a valid location");
    let resource = manager.get(&logs).expect("vanilla has a logs tag");
    assert_eq!(&*resource.source, VANILLA_PACK_ID);
}

#[test]
fn a_pack_in_the_directory_overrides_the_built_in_one() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let datapacks = dir.path().join("datapacks");
    write_pack(&datapacks, "mine");
    write(
        &datapacks.join("mine/data/minecraft/tags/block/logs.json"),
        r#"{"values":["minecraft:sponge"]}"#,
    );

    let repository = PackRepository::discover(datapacks).expect("the built-in pack opens");
    assert_eq!(repository.selected(), [VANILLA_PACK_ID, "file/mine"]);

    let manager = repository.open();
    let logs = Identifier::parse(LOGS).expect("a valid location");

    // The winner is the pack that was found last.
    let winner = manager.get(&logs).expect("both packs have a logs tag");
    assert_eq!(&*winner.source, "file/mine");
    assert!(String::from_utf8_lossy(&winner.data).contains("sponge"));

    // Both copies are still reachable, which is what a tag needs to merge them.
    let stack = manager.get_stack(&logs);
    assert_eq!(stack.len(), 2);
    assert_eq!(&*stack[0].source, VANILLA_PACK_ID);
    assert_eq!(&*stack[1].source, "file/mine");
}

#[test]
fn a_pack_can_add_a_file_the_built_in_one_does_not_have() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let datapacks = dir.path().join("datapacks");
    write_pack(&datapacks, "mine");
    write(
        &datapacks.join("mine/data/mypack/tags/block/shiny.json"),
        r#"{"values":["minecraft:diamond_block"]}"#,
    );

    let repository = PackRepository::discover(datapacks).expect("the built-in pack opens");
    let manager = repository.open();
    assert!(manager.namespaces().contains("mypack"));

    // A resource path is lowercase by definition, so the extension can only be spelled one way.
    #[expect(clippy::case_sensitive_file_extension_comparisons)]
    let listed = manager.list("tags/block", |id| id.path().ends_with(".json"));
    let shiny = Identifier::parse("mypack:tags/block/shiny.json").expect("a valid location");
    assert!(listed.contains_key(&shiny));
    // The listing spans both packs rather than only the one that was asked about.
    let logs = Identifier::parse(LOGS).expect("a valid location");
    assert!(listed.contains_key(&logs));
}

#[test]
fn reloading_picks_up_what_changed_on_disk() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let datapacks = dir.path().join("datapacks");
    let mut repository =
        PackRepository::discover(datapacks.clone()).expect("the built-in pack opens");
    assert_eq!(repository.selected(), [VANILLA_PACK_ID]);

    write_pack(&datapacks, "later");
    write(
        &datapacks.join("later/data/minecraft/tags/block/logs.json"),
        r#"{"values":[]}"#,
    );
    repository.reload().expect("the built-in pack opens");

    assert_eq!(repository.selected(), [VANILLA_PACK_ID, "file/later"]);
    let logs = Identifier::parse(LOGS).expect("a valid location");
    assert_eq!(
        &*repository
            .open()
            .get(&logs)
            .expect("the new pack has a logs tag")
            .source,
        "file/later"
    );

    // And it lets go of a pack that is gone again.
    fs::remove_dir_all(datapacks.join("later")).expect("a removable directory");
    repository.reload().expect("the built-in pack opens");
    assert_eq!(repository.selected(), [VANILLA_PACK_ID]);
}

#[test]
fn a_directory_that_is_not_a_pack_is_left_alone() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let datapacks = dir.path().join("datapacks");
    // No pack.mcmeta, so not a pack.
    write(&datapacks.join("notes/readme.txt"), "not a pack");
    // A pack.mcmeta that says nothing about which version it is for.
    write(&datapacks.join("broken/pack.mcmeta"), r#"{"pack":{}}"#);

    let repository = PackRepository::discover(datapacks).expect("the built-in pack opens");
    assert_eq!(repository.selected(), [VANILLA_PACK_ID]);
}
