//! What the condition language has to do: read what the game writes, and answer it correctly.

use crate::condition::{Condition, Predicates};
use crate::context::{ItemRef, LootContext, LootParams, LootWorld, Origin};
use ferrumc_datapack::manager::FileToId;
use ferrumc_datapack::{Identifier, ResourceManager};
use ferrumc_world::block_state::BlockId;
use ferrumc_world::block_state_id::BlockStateId;
use ferrumc_world::light::LightLayer;
use ferrumc_world::pos::BlockPos;
use rand::SeedableRng;
use serde_json::Value;
use std::sync::Arc;

fn built_in() -> ResourceManager {
    ResourceManager::new(vec![Arc::new(
        ferrumc_datapack::vanilla_pack().expect("the built-in pack opens"),
    )])
}

fn state(name: &str) -> BlockStateId {
    BlockId::from_name(name)
        .unwrap_or_else(|| panic!("{name} should exist"))
        .default_state()
}

fn item(name: &str) -> ItemRef {
    ItemRef {
        id: ferrumc_registry::lookup_item_protocol_id(name)
            .unwrap_or_else(|| panic!("{name} should be an item")),
        count: 1,
    }
}

/// Runs a condition against a bag of parameters, with a fixed roll so a chance is repeatable.
fn holds(json: Value, params: LootParams) -> bool {
    let condition = Condition::parse(&json).expect("a readable condition");
    let mut random = rand::rngs::StdRng::seed_from_u64(1);
    let mut context = LootContext::new(params, &mut random);
    condition.test(&mut context)
}

/// The whole of vanilla's data read, not a handful: every condition in every loot table.
#[test]
fn every_condition_the_game_writes_can_be_read() {
    let manager = built_in();
    let mut conditions = 0;
    let mut tables = 0;
    let mut failures = Vec::new();

    for (id, resource) in FileToId::json("loot_table").list(&manager) {
        tables += 1;
        let table: Value = serde_json::from_slice(&resource.data).expect("a loot table is json");
        walk(&table, &mut |condition| {
            conditions += 1;
            if let Err(e) = Condition::parse(condition) {
                failures.push(format!("{id}: {e}"));
            }
        });
    }

    assert!(tables > 1000, "only {tables} loot tables were read");
    assert!(conditions > 1000, "only {conditions} conditions were read");
    assert!(
        failures.is_empty(),
        "{} conditions could not be read, first few: {:?}",
        failures.len(),
        &failures[..failures.len().min(5)]
    );
}

/// The same for advancements, which gate on the same language.
#[test]
fn every_condition_in_an_advancement_can_be_read() {
    let manager = built_in();
    let mut failures = Vec::new();
    let mut conditions = 0;

    for (id, resource) in FileToId::json("advancement").list(&manager) {
        let advancement: Value =
            serde_json::from_slice(&resource.data).expect("an advancement is json");
        walk(&advancement, &mut |condition| {
            conditions += 1;
            if let Err(e) = Condition::parse(condition) {
                failures.push(format!("{id}: {e}"));
            }
        });
    }

    // Without this the walk finding nothing would read as everything passing.
    assert!(conditions > 300, "only {conditions} conditions were read");
    assert!(
        failures.is_empty(),
        "{} of {conditions} conditions could not be read, first few: {:?}",
        failures.len(),
        &failures[..failures.len().min(5)]
    );
}

/// Every object carrying a `condition` field, wherever it sits.
fn walk(value: &Value, found: &mut impl FnMut(&Value)) {
    match value {
        Value::Object(object) => {
            if object.contains_key("condition") {
                found(value);
            }
            for child in object.values() {
                walk(child, found);
            }
        }
        Value::Array(items) => {
            for child in items {
                walk(child, found);
            }
        }
        _ => {}
    }
}

#[test]
fn a_block_state_property_reads_the_block_being_broken() {
    let condition = serde_json::json!({
        "condition": "minecraft:block_state_property",
        "block": "minecraft:wheat",
        "properties": {"age": "7"}
    });
    let grown = state("minecraft:wheat")
        .with(ferrumc_world::block_state::properties::AGE, 7)
        .expect("wheat grows to 7");

    assert!(holds(
        condition.clone(),
        LootParams {
            block_state: Some(grown),
            ..LootParams::default()
        }
    ));
    assert!(!holds(
        condition.clone(),
        LootParams {
            block_state: Some(state("minecraft:wheat")),
            ..LootParams::default()
        }
    ));
    // Nothing being broken is not the same as something that does not match.
    assert!(!holds(condition, LootParams::default()));
}

#[test]
fn match_tool_reads_the_tool() {
    let condition = serde_json::json!({
        "condition": "minecraft:match_tool",
        "predicate": {"items": "minecraft:shears"}
    });
    assert!(holds(
        condition.clone(),
        LootParams {
            tool: Some(item("minecraft:shears")),
            ..LootParams::default()
        }
    ));
    assert!(!holds(
        condition.clone(),
        LootParams {
            tool: Some(item("minecraft:stone")),
            ..LootParams::default()
        }
    ));
    assert!(
        !holds(condition, LootParams::default()),
        "no tool, no match"
    );
}

#[test]
fn surviving_an_explosion_is_certain_when_there_was_none() {
    let condition = serde_json::json!({"condition": "minecraft:survives_explosion"});
    assert!(holds(condition.clone(), LootParams::default()));

    // A blast makes it a roll, and a bigger one makes it a longer shot: over many rolls a radius
    // of one always survives and a radius of ten mostly does not.
    let parsed = Condition::parse(&condition).expect("a readable condition");
    let mut random = rand::rngs::StdRng::seed_from_u64(3);
    let mut survived = 0;
    for _ in 0..1000 {
        let mut context = LootContext::new(
            LootParams {
                explosion_radius: Some(10.0),
                ..LootParams::default()
            },
            &mut random,
        );
        if parsed.test(&mut context) {
            survived += 1;
        }
    }
    assert!(
        (50..=150).contains(&survived),
        "a radius of ten should let about a tenth through, got {survived}"
    );
}

#[test]
fn composition_nests() {
    let block = |name: &str| {
        serde_json::json!({
            "condition": "minecraft:block_state_property",
            "block": name
        })
    };
    let params = LootParams {
        block_state: Some(state("minecraft:stone")),
        ..LootParams::default()
    };

    assert!(holds(
        serde_json::json!({"condition": "minecraft:any_of", "terms": [block("minecraft:dirt"), block("minecraft:stone")]}),
        params
    ));
    assert!(!holds(
        serde_json::json!({"condition": "minecraft:all_of", "terms": [block("minecraft:dirt"), block("minecraft:stone")]}),
        params
    ));
    assert!(holds(
        serde_json::json!({"condition": "minecraft:inverted", "term": block("minecraft:dirt")}),
        params
    ));
    // A bare list of terms means all of them, which is the inline form vanilla accepts.
    assert!(holds(serde_json::json!([block("minecraft:stone")]), params));
}

#[test]
fn killed_by_player_asks_whether_there_was_one() {
    let condition = serde_json::json!({"condition": "minecraft:killed_by_player"});
    assert!(!holds(condition.clone(), LootParams::default()));
    assert!(holds(
        condition,
        LootParams {
            killed_by_player: true,
            ..LootParams::default()
        }
    ));
}

#[test]
fn a_value_check_works_its_number_out_first() {
    assert!(holds(
        serde_json::json!({
            "condition": "minecraft:value_check",
            "value": {"type": "minecraft:constant", "value": 5},
            "range": {"min": 4, "max": 6}
        }),
        LootParams::default()
    ));
    assert!(!holds(
        serde_json::json!({
            "condition": "minecraft:value_check",
            "value": {"type": "minecraft:constant", "value": 5},
            "range": {"min": 6}
        }),
        LootParams::default()
    ));
}

/// A world of one block, enough for the conditions that ask about a place.
struct Flat {
    time: i64,
    raining: bool,
}

impl LootWorld for Flat {
    fn block_state(&self, _pos: BlockPos) -> Option<BlockStateId> {
        Some(state("minecraft:stone"))
    }
    fn light(&self, _pos: BlockPos, _layer: LightLayer) -> Option<u8> {
        Some(15)
    }
    fn can_see_sky(&self, _pos: BlockPos) -> Option<bool> {
        Some(true)
    }
    fn dimension(&self) -> &str {
        "minecraft:overworld"
    }
    fn time(&self) -> i64 {
        self.time
    }
    fn is_raining(&self) -> bool {
        self.raining
    }
    fn is_thundering(&self) -> bool {
        false
    }
}

fn holds_in(json: Value, params: LootParams, world: &Flat) -> bool {
    let condition = Condition::parse(&json).expect("a readable condition");
    let mut random = rand::rngs::StdRng::seed_from_u64(1);
    let mut context = LootContext::new(params, &mut random).with_world(world);
    condition.test(&mut context)
}

#[test]
fn a_location_check_reads_the_world_at_an_offset() {
    let world = Flat {
        time: 0,
        raining: false,
    };
    let params = LootParams {
        origin: Some(Origin {
            x: 0.0,
            y: 40.0,
            z: 0.0,
        }),
        ..LootParams::default()
    };
    assert!(holds_in(
        serde_json::json!({
            "condition": "minecraft:location_check",
            "predicate": {"position": {"y": {"max": 63}}}
        }),
        params,
        &world
    ));
    // The offset moves the place that is looked at, not the place this happened.
    assert!(!holds_in(
        serde_json::json!({
            "condition": "minecraft:location_check",
            "offsetY": 30,
            "predicate": {"position": {"y": {"max": 63}}}
        }),
        params,
        &world
    ));
}

#[test]
fn a_time_check_folds_by_its_period() {
    let world = Flat {
        time: 24_100,
        raining: false,
    };
    // Past dawn of the second day: within the day, but not within the whole count.
    assert!(holds_in(
        serde_json::json!({
            "condition": "minecraft:time_check",
            "period": 24000,
            "value": {"min": 0, "max": 1000}
        }),
        LootParams::default(),
        &world
    ));
    assert!(!holds_in(
        serde_json::json!({
            "condition": "minecraft:time_check",
            "value": {"min": 0, "max": 1000}
        }),
        LootParams::default(),
        &world
    ));
}

#[test]
fn a_weather_check_reads_the_sky() {
    let dry = Flat {
        time: 0,
        raining: false,
    };
    let wet = Flat {
        time: 0,
        raining: true,
    };
    let condition = serde_json::json!({"condition": "minecraft:weather_check", "raining": true});
    assert!(!holds_in(condition.clone(), LootParams::default(), &dry));
    assert!(holds_in(condition, LootParams::default(), &wet));
}

#[test]
fn a_reference_names_another_predicate() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("data/mypack/predicate/is_stone.json");
    std::fs::create_dir_all(file.parent().expect("a file has a parent")).expect("a writable dir");
    std::fs::write(
        &file,
        r#"{"condition": "minecraft:block_state_property", "block": "minecraft:stone"}"#,
    )
    .expect("a writable file");

    let manager = ResourceManager::new(vec![Arc::new(
        ferrumc_datapack::DirPack::open("test", dir.path().to_path_buf())
            .expect("an openable pack"),
    )]);
    let predicates = Predicates::load(&manager);
    assert_eq!(predicates.len(), 1);
    assert!(predicates
        .get(&Identifier::parse("mypack:is_stone").expect("a valid location"))
        .is_some());

    let condition = Condition::parse(&serde_json::json!({
        "condition": "minecraft:reference",
        "name": "mypack:is_stone"
    }))
    .expect("a readable condition");
    let mut random = rand::rngs::StdRng::seed_from_u64(1);
    let mut context = LootContext::new(
        LootParams {
            block_state: Some(state("minecraft:stone")),
            ..LootParams::default()
        },
        &mut random,
    )
    .with_predicates(&predicates);
    assert!(condition.test(&mut context));
}

/// A reference that names itself is caught rather than followed for ever.
#[test]
fn a_reference_loop_ends() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("data/mypack/predicate/loop.json");
    std::fs::create_dir_all(file.parent().expect("a file has a parent")).expect("a writable dir");
    std::fs::write(
        &file,
        r#"{"condition": "minecraft:reference", "name": "mypack:loop"}"#,
    )
    .expect("a writable file");

    let manager = ResourceManager::new(vec![Arc::new(
        ferrumc_datapack::DirPack::open("test", dir.path().to_path_buf())
            .expect("an openable pack"),
    )]);
    let predicates = Predicates::load(&manager);
    let condition = Condition::parse(&serde_json::json!({
        "condition": "minecraft:reference",
        "name": "mypack:loop"
    }))
    .expect("a readable condition");
    let mut random = rand::rngs::StdRng::seed_from_u64(1);
    let mut context =
        LootContext::new(LootParams::default(), &mut random).with_predicates(&predicates);
    assert!(!condition.test(&mut context));
}

/// An unknown condition is refused rather than quietly treated as holding, which would silently
/// change what a table drops.
#[test]
fn an_unknown_condition_is_refused() {
    assert!(Condition::parse(&serde_json::json!({"condition": "mypack:invented"})).is_err());
    assert!(Condition::parse(&serde_json::json!({})).is_err());
}
