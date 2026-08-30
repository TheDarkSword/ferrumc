//! What recipes have to do: read what the game writes, and match what a player puts down.

use crate::crafting::CraftingInput;
use crate::{CookingKind, Recipe, RecipeBook};
use ferrumc_datapack::tag::TagRegistry;
use ferrumc_datapack::{Identifier, ResourceManager};
use serde_json::Value;
use std::sync::Arc;

fn built_in() -> ResourceManager {
    ResourceManager::new(vec![Arc::new(
        ferrumc_datapack::vanilla_pack().expect("the built-in pack opens"),
    )])
}

fn book() -> RecipeBook {
    RecipeBook::load(&built_in())
}

fn tags() -> Arc<TagRegistry> {
    ferrumc_registry::tags::current().item()
}

fn item(name: &str) -> i32 {
    ferrumc_registry::lookup_item_protocol_id(name)
        .unwrap_or_else(|| panic!("{name} should be an item"))
}

/// A grid written the way a player fills one, with `.` for an empty slot.
fn grid(rows: &[&str], key: &[(char, &str)]) -> CraftingInput {
    let width = rows.first().map_or(0, |row| row.chars().count());
    let items: Vec<Option<i32>> = rows
        .iter()
        .flat_map(|row| row.chars())
        .map(|symbol| {
            if symbol == '.' {
                None
            } else {
                let name = key
                    .iter()
                    .find(|(k, _)| *k == symbol)
                    .unwrap_or_else(|| panic!("'{symbol}' should be in the key"))
                    .1;
                Some(item(name))
            }
        })
        .collect();
    CraftingInput::new(width, rows.len(), &items)
}

/// The whole of vanilla's recipe data read, not a handful.
#[test]
fn every_recipe_the_game_writes_can_be_read() {
    let manager = built_in();
    let mut failures = Vec::new();
    let mut count = 0;

    for (id, resource) in ferrumc_datapack::manager::FileToId::json(crate::DIRECTORY).list(&manager)
    {
        count += 1;
        let value: Value = serde_json::from_slice(&resource.data).expect("a recipe is json");
        if let Err(e) = Recipe::parse(&value) {
            failures.push(format!("{id}: {e}"));
        }
    }

    assert!(count > 1500, "only {count} recipes were read");
    assert!(
        failures.is_empty(),
        "{} of {count} recipes could not be read, first few: {:?}",
        failures.len(),
        &failures[..failures.len().min(5)]
    );
}

#[test]
fn the_built_in_pack_carries_the_vanilla_recipes() {
    let book = book();
    assert!(book.len() > 1500, "found {} recipes", book.len());
    assert!(book
        .get(&Identifier::parse("minecraft:crafting_table").expect("a valid location"))
        .is_some());
}

/// A shape matches wherever it is laid in the grid, because the grid is trimmed to what is in it.
#[test]
fn a_shape_matches_anywhere_in_the_grid() {
    let book = book();
    let tags = tags();
    let planks = [('p', "minecraft:oak_planks")];

    for rows in [
        &["pp.", "pp.", "..."][..],
        &["...", ".pp", ".pp"][..],
        &["pp", "pp"][..],
    ] {
        let made = book
            .match_grid(&tags, &grid(rows, &planks))
            .and_then(Recipe::result)
            .unwrap_or_else(|| panic!("four planks should make something, laid as {rows:?}"));
        assert_eq!(made.item, item("minecraft:crafting_table"));
    }
}

/// A shape that reads differently mirrored still matches either way round.
#[test]
fn a_shape_matches_mirrored() {
    let book = book();
    let tags = tags();
    // A bow is three sticks down one side and three strings curving the other, so it is not its
    // own mirror.
    let key = [('s', "minecraft:stick"), ('t', "minecraft:string")];
    let normal = grid(&[".st", "s.t", ".st"], &key);
    let mirrored = grid(&["ts.", "t.s", "ts."], &key);

    for input in [normal, mirrored] {
        let made = book
            .match_grid(&tags, &input)
            .and_then(Recipe::result)
            .expect("a bow either way round");
        assert_eq!(made.item, item("minecraft:bow"));
    }
}

/// The count on the result is what comes back, not one of it.
#[test]
fn a_recipe_gives_back_the_count_it_says() {
    let book = book();
    let tags = tags();
    let made = book
        .match_grid(&tags, &grid(&["p", "p"], &[('p', "minecraft:oak_planks")]))
        .and_then(Recipe::result)
        .expect("two planks make sticks");
    assert_eq!(made.item, item("minecraft:stick"));
    assert_eq!(made.count, 4);
}

/// An ingredient written as a tag takes anything in it.
#[test]
fn an_ingredient_can_be_a_tag() {
    let book = book();
    let tags = tags();
    for planks in ["minecraft:oak_planks", "minecraft:spruce_planks"] {
        let made = book
            .match_grid(&tags, &grid(&["pp", "pp"], &[('p', planks)]))
            .and_then(Recipe::result)
            .unwrap_or_else(|| panic!("{planks} should make a crafting table"));
        assert_eq!(made.item, item("minecraft:crafting_table"));
    }
}

/// A shapeless recipe takes its ingredients in any arrangement, and only the right ones.
#[test]
fn a_shapeless_recipe_ignores_the_arrangement() {
    let book = book();
    let tags = tags();
    for rows in [&["l.", ".."][..], &[".l", ".."][..], &["l"][..]] {
        let made = book
            .match_grid(&tags, &grid(rows, &[('l', "minecraft:oak_log")]))
            .and_then(Recipe::result)
            .unwrap_or_else(|| panic!("a log makes planks, laid as {rows:?}"));
        assert_eq!(made.item, item("minecraft:oak_planks"));
        assert_eq!(made.count, 4);
    }
}

/// A grid holding more than a recipe asks for does not match it.
#[test]
fn a_spare_item_spoils_the_match() {
    let book = book();
    let tags = tags();
    let made = book.match_grid(
        &tags,
        &grid(
            &["pp", "ps"],
            &[('p', "minecraft:oak_planks"), ('s', "minecraft:stone")],
        ),
    );
    assert!(made.is_none(), "three planks and a stone make nothing");
}

/// Two ingredients that could each take either of two slots must still both be satisfied, which a
/// walk that took the first slot it liked would get wrong.
#[test]
fn a_shapeless_match_is_a_matching_rather_than_a_walk() {
    let recipe = Recipe::parse(&serde_json::json!({
        "type": "minecraft:crafting_shapeless",
        "ingredients": ["#minecraft:planks", "minecraft:oak_planks"],
        "result": {"id": "minecraft:stick"}
    }))
    .expect("a readable recipe");
    let tags = tags();

    // The tag ingredient could take the oak, which would leave the second with only spruce.
    let input = grid(
        &["os"],
        &[
            ('o', "minecraft:oak_planks"),
            ('s', "minecraft:spruce_planks"),
        ],
    );
    assert!(recipe.matches_grid(&tags, &input));

    // Two spruce cannot satisfy the ingredient that wants oak.
    let input = grid(&["ss"], &[('s', "minecraft:spruce_planks")]);
    assert!(!recipe.matches_grid(&tags, &input));
}

/// Cooking recipes carry their time and experience, and the default differs by appliance.
#[test]
fn cooking_resolves_its_time_and_experience() {
    let book = book();
    let tags = tags();

    let Some(Recipe::Cooking {
        cooking_time,
        experience,
        result,
        ..
    }) = book.match_cooking(&tags, CookingKind::Furnace, item("minecraft:potato"))
    else {
        panic!("a potato bakes in a furnace");
    };
    assert_eq!(result.item, item("minecraft:baked_potato"));
    assert_eq!(*cooking_time, 200, "a furnace takes two hundred ticks");
    assert!((*experience - 0.35).abs() < 1e-6, "got {experience}");

    // The same food in a smoker takes half as long.
    let Some(Recipe::Cooking { cooking_time, .. }) =
        book.match_cooking(&tags, CookingKind::Smoker, item("minecraft:potato"))
    else {
        panic!("a potato bakes in a smoker too");
    };
    assert_eq!(*cooking_time, 100);
}

/// A furnace recipe is not a blast furnace recipe.
#[test]
fn cooking_is_told_apart_by_appliance() {
    let book = book();
    let tags = tags();
    assert!(book
        .match_cooking(&tags, CookingKind::BlastFurnace, item("minecraft:potato"))
        .is_none());
    assert!(book
        .match_cooking(&tags, CookingKind::BlastFurnace, item("minecraft:raw_iron"))
        .is_some());
}

/// A stonecutter offers everything a block can be cut into.
#[test]
fn stonecutting_offers_several_things() {
    let book = book();
    let tags = tags();
    let cuts: Vec<i32> = book
        .match_stonecutting(&tags, item("minecraft:stone"))
        .filter_map(Recipe::result)
        .map(|result| result.item)
        .collect();
    assert!(
        cuts.len() > 3,
        "stone cuts several ways, found {}",
        cuts.len()
    );
    assert!(cuts.contains(&item("minecraft:stone_stairs")));
}

/// A recipe type the game does not have is refused rather than quietly dropped.
#[test]
fn an_unknown_recipe_type_is_refused() {
    assert!(Recipe::parse(&serde_json::json!({"type": "mypack:invented"})).is_err());
    assert!(Recipe::parse(&serde_json::json!({})).is_err());
}

/// A pack can add a recipe, and it is matched like any other.
#[test]
fn a_pack_can_add_a_recipe() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("data/mypack/recipe/stone_from_dirt.json");
    std::fs::create_dir_all(file.parent().expect("a file has a parent")).expect("a writable dir");
    std::fs::write(
        &file,
        r#"{"type":"minecraft:crafting_shapeless","ingredients":["minecraft:dirt"],
            "result":{"id":"minecraft:stone","count":2}}"#,
    )
    .expect("a writable file");

    let stack = ResourceManager::new(vec![
        Arc::new(ferrumc_datapack::vanilla_pack().expect("the built-in pack opens")),
        Arc::new(
            ferrumc_datapack::DirPack::open("test", dir.path().to_path_buf())
                .expect("an openable pack"),
        ),
    ]);
    let book = RecipeBook::load(&stack);
    let made = book
        .match_grid(&tags(), &grid(&["d"], &[('d', "minecraft:dirt")]))
        .and_then(Recipe::result)
        .expect("the pack's recipe");
    assert_eq!(made.item, item("minecraft:stone"));
    assert_eq!(made.count, 2);
}
