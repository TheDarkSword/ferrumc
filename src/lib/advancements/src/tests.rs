//! What advancements have to do: read what the game writes, track what a player has done, and lay
//! the trees out so the screen can draw them.

use crate::progress::{Award, PlayerAdvancements};
use crate::{Advancement, Advancements};
use ferrumc_datapack::ResourceManager;
use serde_json::Value;
use std::sync::Arc;

fn built_in() -> ResourceManager {
    ResourceManager::new(vec![Arc::new(
        ferrumc_datapack::vanilla_pack().expect("the built-in pack opens"),
    )])
}

fn loaded() -> Advancements {
    Advancements::load(&built_in())
}

/// The whole of vanilla's advancement data read, not a handful.
#[test]
fn every_advancement_the_game_writes_can_be_read() {
    let manager = built_in();
    let mut failures = Vec::new();
    let mut count = 0;

    for (id, resource) in ferrumc_datapack::manager::FileToId::json(crate::DIRECTORY).list(&manager)
    {
        count += 1;
        let value: Value = serde_json::from_slice(&resource.data).expect("an advancement is json");
        if let Err(e) = Advancement::parse(&value) {
            failures.push(format!("{id}: {e}"));
        }
    }

    assert!(count > 1600, "only {count} advancements were read");
    assert!(
        failures.is_empty(),
        "{} of {count} could not be read, first few: {:?}",
        failures.len(),
        &failures[..failures.len().min(5)]
    );
}

#[test]
fn the_built_in_pack_carries_the_vanilla_advancements() {
    let advancements = loaded();
    assert!(advancements.len() > 1600, "found {}", advancements.len());

    let root = advancements
        .get_by_name("minecraft:story/root")
        .expect("the story root");
    assert!(root.is_root());
    assert!(root.display.is_some(), "the story root is shown");

    // Most of them are the hidden ones that unlock a recipe.
    let hidden = advancements
        .iter()
        .filter(|(_, advancement)| advancement.display.is_none())
        .count();
    assert!(hidden > 1500, "only {hidden} are hidden");
}

/// Requirements are an and of ors: every group needs one of its criteria.
#[test]
fn requirements_are_an_and_of_ors() {
    let advancement = Advancement::parse(&serde_json::json!({
        "criteria": {
            "a": {"trigger": "minecraft:impossible"},
            "b": {"trigger": "minecraft:impossible"},
            "c": {"trigger": "minecraft:impossible"}
        },
        "requirements": [["a", "b"], ["c"]]
    }))
    .expect("a readable advancement");

    let met = |granted: &[&str]| advancement.requirements.met(|name| granted.contains(&name));
    assert!(!met(&["a"]), "the second group is unmet");
    assert!(!met(&["c"]), "the first group is unmet");
    assert!(met(&["a", "c"]));
    assert!(met(&["b", "c"]));
}

/// With nothing said, every criterion is its own group, so all of them are needed.
#[test]
fn requirements_default_to_all_of_them() {
    let advancement = Advancement::parse(&serde_json::json!({
        "criteria": {
            "a": {"trigger": "minecraft:impossible"},
            "b": {"trigger": "minecraft:impossible"}
        }
    }))
    .expect("a readable advancement");
    assert!(!advancement.requirements.met(|name| name == "a"));
    assert!(advancement.requirements.met(|_| true));
}

/// Granting a criterion says whether it finished the advancement, and only the first time.
#[test]
fn granting_reports_completion_once() {
    let advancements = loaded();
    let mut player = PlayerAdvancements::default();
    let name = "minecraft:story/root";

    assert_eq!(
        player.award(&advancements, name, "crafting_table", 1),
        Award::Completed
    );
    assert!(player.is_done(&advancements, name));
    assert_eq!(
        player.award(&advancements, name, "crafting_table", 2),
        Award::Already
    );
}

/// A criterion taken back leaves the advancement unfinished, and an empty entry is not kept.
#[test]
fn revoking_undoes_it() {
    let advancements = loaded();
    let mut player = PlayerAdvancements::default();
    let name = "minecraft:story/root";

    player.award(&advancements, name, "crafting_table", 1);
    assert!(player.revoke(name, "crafting_table"));
    assert!(!player.is_done(&advancements, name));
    assert!(player.get(name).is_none(), "nothing is kept for it");
}

/// What a player has done survives being written out and read back, which is what persistence
/// across a restart is.
#[test]
fn progress_survives_a_round_trip() {
    let advancements = loaded();
    let mut player = PlayerAdvancements::default();
    player.award(&advancements, "minecraft:story/root", "crafting_table", 42);

    let written = bitcode_round_trip(&player);
    assert!(written.is_done(&advancements, "minecraft:story/root"));
    assert_eq!(
        written
            .get("minecraft:story/root")
            .and_then(|progress| progress.criteria.get("crafting_table")),
        Some(&42),
        "when it was earned comes back too"
    );
}

/// Written the way the world writes it, which is what a restart reads back.
fn bitcode_round_trip(player: &PlayerAdvancements) -> PlayerAdvancements {
    bitcode::decode(&bitcode::encode(player)).expect("progress reads back")
}

/// An advancement with an unknown trigger is refused rather than quietly never firing.
#[test]
fn an_unknown_trigger_is_refused() {
    assert!(Advancement::parse(&serde_json::json!({
        "criteria": {"a": {"trigger": "mypack:invented"}}
    }))
    .is_err());
    // As is one with no criteria at all, which could never be finished.
    assert!(Advancement::parse(&serde_json::json!({"criteria": {}})).is_err());
    // And one whose requirements name a criterion it does not have.
    assert!(Advancement::parse(&serde_json::json!({
        "criteria": {"a": {"trigger": "minecraft:impossible"}},
        "requirements": [["b"]]
    }))
    .is_err());
}

/// The trees are laid out so the screen can draw them: depth along one axis, siblings along the
/// other, and no two advancements of a tree in the same place.
#[test]
fn the_trees_are_laid_out_without_overlap() {
    let advancements = loaded();

    // Every root's own tree, since each is a tab of its own.
    for root in ["minecraft:story/root", "minecraft:nether/root"] {
        let mut places = Vec::new();
        for (name, advancement) in advancements.iter() {
            if advancement.display.is_none() {
                continue;
            }
            // Everything under this root, by walking up to it.
            let mut at = name;
            let mut top = name;
            while let Some(parent) = advancements.get_by_name(at).and_then(|a| a.parent.as_ref()) {
                at = parent.as_str();
                top = at;
            }
            if top == root {
                places.push((name, advancements.position(name)));
            }
        }
        assert!(places.len() > 5, "{root} has {} shown", places.len());

        let mut seen = std::collections::BTreeSet::new();
        for (name, (x, y)) in &places {
            let at = (*x as i32, (*y * 100.0) as i32);
            assert!(seen.insert(at), "{name} sits on top of another at {at:?}");
        }
        // The root is at the left, and something is further right.
        let root_x = advancements.position(root).0;
        assert!(places.iter().any(|(_, (x, _))| *x > root_x));
    }
}

/// A parent sits level with the middle of its children, which is what makes the tree read as one.
#[test]
fn a_parent_sits_between_its_children() {
    let advancements = loaded();
    let mut checked = 0;

    for (name, advancement) in advancements.iter() {
        if advancement.display.is_none() {
            continue;
        }
        let children: Vec<f32> = advancements
            .iter()
            .filter(|(_, child)| {
                child.display.is_some()
                    && child
                        .parent
                        .as_ref()
                        .is_some_and(|parent| parent.as_str() == name)
            })
            .map(|(child, _)| advancements.position(child).1)
            .collect();
        if children.len() < 2 {
            continue;
        }
        let (top, bottom) = (
            children.iter().copied().fold(f32::MAX, f32::min),
            children.iter().copied().fold(f32::MIN, f32::max),
        );
        let parent = advancements.position(name).1;
        assert!(
            parent >= top - 0.001 && parent <= bottom + 0.001,
            "{name} sits at {parent}, outside its children's {top} to {bottom}"
        );
        checked += 1;
    }
    assert!(checked > 5, "only {checked} had children to sit between");
}

/// A hidden advancement takes no place on the tree, and its children hang off its parent.
#[test]
fn the_recipe_tree_is_not_laid_out() {
    let advancements = loaded();
    // Its root is invisible, so nothing under it is placed.
    assert_eq!(advancements.position("minecraft:recipes/root"), (0.0, 0.0));
}
