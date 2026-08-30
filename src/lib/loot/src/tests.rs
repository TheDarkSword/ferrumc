//! What loot tables have to do: read what the game writes, and drop what it drops.

use crate::{ItemStack, LootTable, LootTables};
use ferrumc_datapack::{Identifier, ResourceManager};
use ferrumc_predicates::{LootContext, LootParams};
use ferrumc_world::block_state::BlockId;
use rand::SeedableRng;
use serde_json::Value;
use std::sync::Arc;

fn built_in() -> ResourceManager {
    ResourceManager::new(vec![Arc::new(
        ferrumc_datapack::vanilla_pack().expect("the built-in pack opens"),
    )])
}

fn tables() -> LootTables {
    LootTables::load(&built_in())
}

fn id(name: &str) -> Identifier {
    Identifier::parse(name).expect("a valid location")
}

fn item(name: &str) -> i32 {
    ferrumc_registry::lookup_item_protocol_id(name)
        .unwrap_or_else(|| panic!("{name} should be an item"))
}

fn broken(name: &str) -> LootParams {
    LootParams {
        block_state: Some(
            BlockId::from_name(name)
                .unwrap_or_else(|| panic!("{name} should exist"))
                .default_state(),
        ),
        ..LootParams::default()
    }
}

/// Rolls a table many times, counting what came out.
fn sample(
    tables: &LootTables,
    table: &str,
    params: LootParams,
    rolls: usize,
) -> Vec<Vec<ItemStack>> {
    let mut random = rand::rngs::StdRng::seed_from_u64(20260830);
    (0..rolls)
        .map(|_| {
            let mut context = LootContext::new(params, &mut random);
            tables.roll(&id(table), &mut context)
        })
        .collect()
}

/// The whole of vanilla's loot data read, not a handful.
#[test]
fn every_table_the_game_writes_can_be_read() {
    let manager = built_in();
    let mut failures = Vec::new();
    let mut count = 0;

    for (id, resource) in ferrumc_datapack::manager::FileToId::json(crate::DIRECTORY).list(&manager)
    {
        count += 1;
        let value: Value = serde_json::from_slice(&resource.data).expect("a loot table is json");
        if let Err(e) = LootTable::parse(&value) {
            failures.push(format!("{id}: {e}"));
        }
    }

    assert!(count > 1000, "only {count} loot tables were read");
    assert!(
        failures.is_empty(),
        "{} of {count} tables could not be read, first few: {:?}",
        failures.len(),
        &failures[..failures.len().min(5)]
    );
}

#[test]
fn the_built_in_pack_carries_the_vanilla_tables() {
    let tables = tables();
    assert!(tables.len() > 1000, "found {} tables", tables.len());
    assert!(tables.get(&id("minecraft:blocks/stone")).is_some());
}

/// A block whose table is one item, always.
#[test]
fn breaking_a_plain_block_drops_it() {
    let tables = tables();
    for rolled in sample(
        &tables,
        "minecraft:blocks/dirt",
        broken("minecraft:dirt"),
        20,
    ) {
        assert_eq!(rolled.len(), 1, "dirt drops one thing");
        assert_eq!(rolled[0].item, item("minecraft:dirt"));
        assert_eq!(rolled[0].count, 1);
    }
}

/// Stone drops cobblestone, which is the table saying the block and the drop need not agree.
#[test]
fn a_block_can_drop_something_else() {
    let tables = tables();
    let rolled = sample(
        &tables,
        "minecraft:blocks/stone",
        broken("minecraft:stone"),
        1,
    );
    assert_eq!(rolled[0].len(), 1);
    assert_eq!(rolled[0][0].item, item("minecraft:cobblestone"));
}

/// Redstone ore drops four to five, which is a `uniform` roll read through `set_count`.
#[test]
fn an_ore_drops_a_rolled_count() {
    let tables = tables();
    let mut seen = std::collections::BTreeSet::new();
    for rolled in sample(
        &tables,
        "minecraft:blocks/redstone_ore",
        broken("minecraft:redstone_ore"),
        400,
    ) {
        assert_eq!(rolled.len(), 1, "one kind of thing");
        assert_eq!(rolled[0].item, item("minecraft:redstone"));
        seen.insert(rolled[0].count);
    }
    assert_eq!(
        seen.iter().copied().collect::<Vec<_>>(),
        [4, 5],
        "redstone ore drops four or five"
    );
}

/// Leaves are mostly nothing: a five percent chance of a sapling, and nothing otherwise, because
/// the shears and silk touch branches cannot be taken with no tool.
#[test]
fn leaves_mostly_drop_nothing() {
    let tables = tables();
    let rolls = 4000;
    let saplings = sample(
        &tables,
        "minecraft:blocks/oak_leaves",
        broken("minecraft:oak_leaves"),
        rolls,
    )
    .iter()
    .filter(|rolled| {
        rolled
            .iter()
            .any(|stack| stack.item == item("minecraft:oak_sapling"))
    })
    .count();

    // One in twenty, give or take: the range is wide enough that a run of luck does not fail it
    // and narrow enough that a wrong chance would.
    let expected = rolls / 20;
    assert!(
        (expected * 3 / 5..=expected * 7 / 5).contains(&saplings),
        "about {expected} saplings in {rolls}, got {saplings}"
    );
}

/// A blast destroys most of what would have dropped, item by item.
#[test]
fn an_explosion_takes_most_of_the_drop() {
    let tables = tables();
    let rolls = 2000;
    let params = LootParams {
        explosion_radius: Some(5.0),
        ..broken("minecraft:redstone_ore")
    };
    let total: i32 = sample(&tables, "minecraft:blocks/redstone_ore", params, rolls)
        .iter()
        .flatten()
        .map(|stack| stack.count)
        .sum();

    // Four or five redstone, each with a one in five chance of surviving a radius of five.
    let expected = (rolls as i32) * 45 / 10 / 5;
    assert!(
        (expected * 4 / 5..=expected * 6 / 5).contains(&total),
        "about {expected} redstone through the blast, got {total}"
    );
    // Without the blast, all of it comes through.
    let unharmed: i32 = sample(
        &tables,
        "minecraft:blocks/redstone_ore",
        broken("minecraft:redstone_ore"),
        rolls,
    )
    .iter()
    .flatten()
    .map(|stack| stack.count)
    .sum();
    assert!(unharmed > total * 3, "{unharmed} unharmed against {total}");
}

/// Weights decide how often each entry is drawn.
#[test]
fn a_weighted_pool_is_drawn_in_proportion() {
    let table = LootTable::parse(&serde_json::json!({
        "pools": [{
            "rolls": 1,
            "entries": [
                {"type": "minecraft:item", "name": "minecraft:stone", "weight": 3},
                {"type": "minecraft:item", "name": "minecraft:dirt", "weight": 1}
            ]
        }]
    }))
    .expect("a readable table");

    let mut tables = LootTables::default();
    tables.insert(id("test:weighted"), table);

    let rolls = 4000;
    let stone = sample(&tables, "test:weighted", LootParams::default(), rolls)
        .iter()
        .filter(|rolled| rolled[0].item == item("minecraft:stone"))
        .count();
    let expected = rolls * 3 / 4;
    assert!(
        (expected * 9 / 10..=expected * 11 / 10).contains(&stone),
        "about {expected} stone in {rolls}, got {stone}"
    );
}

/// `alternatives` takes the first child that can run, and nothing after it.
#[test]
fn alternatives_stop_at_the_first_that_holds() {
    let table = LootTable::parse(&serde_json::json!({
        "pools": [{
            "rolls": 1,
            "entries": [{
                "type": "minecraft:alternatives",
                "children": [
                    {
                        "type": "minecraft:item",
                        "name": "minecraft:stone",
                        "conditions": [{
                            "condition": "minecraft:block_state_property",
                            "block": "minecraft:dirt"
                        }]
                    },
                    {"type": "minecraft:item", "name": "minecraft:dirt"}
                ]
            }]
        }]
    }))
    .expect("a readable table");
    let mut tables = LootTables::default();
    tables.insert(id("test:alternatives"), table);

    // The first child wants dirt to have been broken, so breaking stone falls through to the
    // second.
    let rolled = sample(&tables, "test:alternatives", broken("minecraft:stone"), 1);
    assert_eq!(rolled[0][0].item, item("minecraft:dirt"));

    let rolled = sample(&tables, "test:alternatives", broken("minecraft:dirt"), 1);
    assert_eq!(rolled[0][0].item, item("minecraft:stone"));
}

/// A table that names another rolls it in place.
#[test]
fn a_nested_table_is_rolled_where_it_sits() {
    let inner = LootTable::parse(&serde_json::json!({
        "pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:stone"}]}]
    }))
    .expect("a readable table");
    let outer = LootTable::parse(&serde_json::json!({
        "pools": [{
            "rolls": 1,
            "entries": [{"type": "minecraft:loot_table", "value": "test:inner"}]
        }]
    }))
    .expect("a readable table");

    let mut tables = LootTables::default();
    tables.insert(id("test:inner"), inner);
    tables.insert(id("test:outer"), outer);

    let rolled = sample(&tables, "test:outer", LootParams::default(), 1);
    assert_eq!(rolled[0][0].item, item("minecraft:stone"));
}

/// A table that names itself is caught rather than followed for ever.
#[test]
fn a_table_loop_ends() {
    let table = LootTable::parse(&serde_json::json!({
        "pools": [{
            "rolls": 1,
            "entries": [{"type": "minecraft:loot_table", "value": "test:loop"}]
        }]
    }))
    .expect("a readable table");
    let mut tables = LootTables::default();
    tables.insert(id("test:loop"), table);

    let rolled = sample(&tables, "test:loop", LootParams::default(), 1);
    assert!(rolled[0].is_empty());
}

/// A pool's conditions gate the whole pool, not each entry.
#[test]
fn a_pool_condition_gates_every_roll() {
    let table = LootTable::parse(&serde_json::json!({
        "pools": [{
            "rolls": 1,
            "conditions": [{
                "condition": "minecraft:block_state_property",
                "block": "minecraft:dirt"
            }],
            "entries": [{"type": "minecraft:item", "name": "minecraft:stone"}]
        }]
    }))
    .expect("a readable table");
    let mut tables = LootTables::default();
    tables.insert(id("test:gated"), table);

    assert!(sample(&tables, "test:gated", broken("minecraft:stone"), 1)[0].is_empty());
    assert_eq!(
        sample(&tables, "test:gated", broken("minecraft:dirt"), 1)[0].len(),
        1
    );
}

/// A chest table gives a plausible spread rather than the same thing every time.
#[test]
fn a_chest_table_gives_a_spread() {
    let tables = tables();
    let mut kinds = std::collections::BTreeSet::new();
    let mut total = 0;
    for rolled in sample(
        &tables,
        "minecraft:chests/simple_dungeon",
        LootParams::default(),
        200,
    ) {
        total += rolled.len();
        for stack in rolled {
            kinds.insert(stack.item);
        }
    }
    assert!(
        total > 200,
        "a dungeon chest holds several things, got {total}"
    );
    assert!(kinds.len() > 5, "only {} kinds of thing", kinds.len());
}

/// An unknown entry or function is refused rather than quietly dropped, which would change what a
/// table gives without saying so.
#[test]
fn an_unknown_piece_is_refused() {
    assert!(LootTable::parse(&serde_json::json!({
        "pools": [{"rolls": 1, "entries": [{"type": "mypack:invented"}]}]
    }))
    .is_err());
    assert!(LootTable::parse(&serde_json::json!({
        "pools": [{
            "rolls": 1,
            "entries": [{
                "type": "minecraft:item",
                "name": "minecraft:stone",
                "functions": [{"function": "mypack:invented"}]
            }]
        }]
    }))
    .is_err());
}
