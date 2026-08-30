# Entities

## A type is an enum

Every entity type the game has is a variant, and the variants carry the registry's **own numbers**:

```rust
EntityType::Pig as u16   // the number that goes on the wire
```

Nothing is looked up to send one, and nothing can drift: a test holds all 158 against the version's
`registries.json`. That matters more than it sounds — the table this replaced had 131 of its 151 ids
wrong, so every mob reached the client as whatever sat at the old index, and the code compiled and
the tests passed throughout.

Everything the game says about a type is one index away from the variant: how big it is, where its
eyes are, which category it spawns in, how far a client is told about it, how often it is updated,
what a living one starts with, and where it may stand.

```rust
EntityType::Pig.max_health()            // Some(10.0)
EntityType::Pig.tracking_range()        // 10 chunks
EntityType::Painting.update_interval()  // None — it never moves, so it is never sent again
```

None of that is in any report the game publishes; it lives on the type in code, so
`scripts/extract_entity_types.py` asks the game and `scripts/build_entity_types.py` writes the enum.

## What an entity carries

The type is the component. Beyond it an entity carries only what actually differs between two of the
same kind — where it is, how fast, whether it is a baby, how long it is still invulnerable for.

How big an entity is is asked of the type rather than kept on the entity. A young one is half a grown
one, except where the type's size is fixed: vanilla refuses to scale one of those, so a baby slime is
not a smaller slime.

## What decides which physics apply

The game's own answer, not a list: a type that belongs on the ground falls and is slowed by water,
one that swims or flies does not fall, and one that lives in lava falls without the water drag. That
is `SpawnPlacement`, which is what vanilla checks before it puts a mob anywhere.

## Markers

A type-specific marker component (`Pig`, `Cow`, …) is inserted alongside the type so a system can
filter an archetype rather than test a value, which is what mob behaviour will want.

## Regenerating

```bash
scripts/extract_entity_types.py   # asks the game; needs 26.1 or newer
scripts/build_entity_types.py     # writes the enum and its table
```
