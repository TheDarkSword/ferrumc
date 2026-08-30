//! What the worldgen definitions have to do: read what the game writes.

use crate::biome::Biome;
use crate::feature::ConfiguredFeature;
use crate::placement::PlacedFeature;
use crate::structure::{Structure, StructureSet};
use ferrumc_datapack::manager::FileToId;
use ferrumc_datapack::ResourceManager;
use serde_json::Value;
use std::sync::Arc;

fn built_in() -> ResourceManager {
    ResourceManager::new(vec![Arc::new(
        ferrumc_datapack::vanilla_pack().expect("the built-in pack opens"),
    )])
}

/// Reads every file of a kind and reports the ones that could not be.
fn read_all(kind: &str, parse: impl Fn(&Value) -> Option<()>) -> (usize, Vec<String>) {
    let manager = built_in();
    let mut failures = Vec::new();
    let mut count = 0;
    for (id, resource) in FileToId::json(format!("worldgen/{kind}")).list(&manager) {
        count += 1;
        let value: Value = serde_json::from_slice(&resource.data).expect("worldgen data is json");
        if parse(&value).is_none() {
            failures.push(id.to_string());
        }
    }
    (count, failures)
}

#[test]
fn every_configured_feature_the_game_writes_can_be_read() {
    let (count, failures) = read_all("configured_feature", |value| {
        ConfiguredFeature::parse(value).map(|_| ())
    });
    assert!(count > 200, "only {count} features were read");
    assert!(
        failures.is_empty(),
        "{} of {count} could not be read: {:?}",
        failures.len(),
        &failures[..failures.len().min(8)]
    );
}

#[test]
fn every_placed_feature_the_game_writes_can_be_read() {
    let (count, failures) = read_all("placed_feature", |value| {
        PlacedFeature::parse(value).map(|_| ())
    });
    assert!(count > 250, "only {count} placements were read");
    assert!(
        failures.is_empty(),
        "{} of {count} could not be read: {:?}",
        failures.len(),
        &failures[..failures.len().min(8)]
    );
}

#[test]
fn every_biome_the_game_writes_can_be_read() {
    let (count, failures) = read_all("biome", |value| Biome::parse(value).map(|_| ()));
    assert!(count > 60, "only {count} biomes were read");
    assert!(
        failures.is_empty(),
        "{} of {count} could not be read: {:?}",
        failures.len(),
        &failures[..failures.len().min(8)]
    );
}

#[test]
fn every_structure_the_game_writes_can_be_read() {
    let (count, failures) = read_all("structure", |value| Structure::parse(value).map(|_| ()));
    assert!(count > 30, "only {count} structures were read");
    assert!(failures.is_empty(), "could not read: {failures:?}");

    let (count, failures) = read_all("structure_set", |value| {
        StructureSet::parse(value).map(|_| ())
    });
    assert!(count > 15, "only {count} structure sets were read");
    assert!(failures.is_empty(), "could not read: {failures:?}");
}

/// A biome carries its features in rounds, and everything in a round happens before the next.
#[test]
fn a_biome_carries_its_features_in_rounds() {
    let manager = built_in();
    let plains = FileToId::json("worldgen/biome")
        .list(&manager)
        .into_iter()
        .find(|(id, _)| id.as_str() == "minecraft:plains")
        .map(|(_, resource)| resource)
        .expect("the plains");
    let biome = Biome::parse(&serde_json::from_slice(&plains.data).expect("a biome is json"))
        .expect("a readable biome");

    assert_eq!(biome.features.len(), crate::biome::DECORATION_STEPS);
    assert!(biome.has_precipitation);
    assert!(!biome.carvers.named().is_empty(), "the plains are carved");
    assert!(
        biome.spawners.values().any(|group| !group.is_empty()),
        "something lives in the plains"
    );
}
