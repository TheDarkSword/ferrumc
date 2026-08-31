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

## What a client is told

Beside the type, every entity carries a `SyncedData` — a short row of values a client reads to draw
it. Flags, health, name, pose. Each sits at an index and travels tagged with the number of the kind
of value that follows.

```rust
data.set_flag(EntityFlag::Crouching, true);
data.set(fields::living_entity::HEALTH, 12.0);
data.get(fields::entity::POSE);            // Some(&Pose::Standing)
```

Fields are named, never numbered, and each carries the shape it holds, so a health cannot be written
where a pose is read. They are grouped by the class that declares them — `fields::entity`,
`fields::living_entity`, `fields::zombie` — because that is what fixes where a field sits: vanilla
hands out indices by walking the entity class tree, so a field of a given name is in the same place
on every type that inherits it.

Writing a value that is already there is not a change. Systems write as they go without caring who
is watching, and one system at the end of the tick sends whatever ended up changed, once. That is
also what stopped three systems clobbering each other: crouching, sprinting and swimming share one
byte, and each used to send the whole byte with only its own bit set.

### Getting it wrong is silent

A field's place and its kind's number are both version-dependent and neither appears in any report
the game publishes. A wrong number does not drop a value — the client reads the bytes as whatever it
does keep at that place. So both are extracted rather than transcribed, and the table is per version:
26.2 made a slime an ageable mob, which pushed its size two places down the row, so a 26.1 client is
told 16 where a 26.2 client is told 18, and a field 26.1 never had is left out rather than misplaced.

Only 26.1 and 26.2 can be asked — older jars are obfuscated — so every older client is currently
sent 26.1's layout. See `internal_docs/deferred.md`.

### What is not modelled

Twenty-one kinds of value have a shape. The rest — particles, villager data, the variant holders,
block states, profiles — are written back exactly as the game wrote their defaults, which is right
until something wants to set one.

## Regenerating

```bash
scripts/extract_entity_types.py   # asks the game what a type is; needs 26.1 or newer
scripts/build_entity_types.py     # writes the enum and its table

scripts/extract_synched_data.py 26.1   # asks the game how a row is laid out
scripts/extract_synched_data.py 26.2
scripts/build_synced_data.py           # writes the layouts, the field names and the vocabularies
```
