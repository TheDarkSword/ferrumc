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
the game publishes. A wrong number does not drop a value: the kind says how many bytes follow, so a
client reads the value as whatever kind it does keep at that number and then reads the rest of the
row at the wrong offset. Both are therefore extracted rather than transcribed, and both are per
version.

**The kind numbers** are read out of every version's own jar. Older jars carry no names, but Mojang
publishes a mapping file for every release, and the registration order is plain to see in the static
initialiser — so `scripts/extract_serializer_ids.py` reads the class file rather than running it, and
neither executes nor remaps anything. A pose is 21 in 1.21 and 20 in 26.2; ViaVersion's own tables
agree with all ten readings, which is a second pair of eyes on numbers that fail silently.

**A field's place** comes from walking the entity class tree, and is read the same way: a class's
static initialiser defines its fields in order, its header names what it extends, and the class a
type is built from is named behind its factory. `scripts/extract_field_layouts.py` puts those three
together for every type of every version, and for the two versions that can also be asked directly
it answers exactly what the running game answers — which is what makes the readings for the other
eight worth trusting.

Between forty and fifty-five types sit differently in each version before 26.1, the player among
them: 26.1 put which hand a player favours in front of absorption and score, so a score written at
18 is read at 16 by everything older. A field an older version has no place for is left out rather
than misplaced.

### What is not modelled

Twenty-one kinds of value have a shape. The rest — particles, villager data, the variant holders,
block states, profiles — are written back exactly as the game wrote their defaults, which is right
until something wants to set one.

## How an entity moves

Two numbers move an entity on its own: the pull downwards and what the medium around it takes back.
Both are per type, and both come from the game rather than from a constant that fits most of them.

```rust
EntityType::Zombie.motion().gravity   // 0.08
EntityType::Item.motion().gravity     // 0.04 — half
EntityType::Arrow.motion().gravity    // 0.05
EntityType::Painting.motion().gravity // 0.0 — it hangs where it is put
```

Sixty-five of the hundred and fifty-eight types are pulled down at something other than 0.08, so a
single constant was wrong for two entities in five. A squid is pulled down as hard as a zombie —
it is the water that holds it up, not a lack of weight — which is the kind of thing a hand-written
table gets wrong and an extractor does not.

### The order is the whole thing

Vanilla pulls the two kinds of entity down at different points in the tick:

- a **mob** moves with what the tick before left it, and is pulled down and slowed afterwards;
- a **dropped thing** is pulled down first, moves with that, and is slowed afterwards.

Both settle at the same speed, so a terminal-velocity check passes either way. What differs is the
first second: in twenty ticks vanilla drops a mob 13.2512 blocks and an item 7.4256. Applying the
pull on the wrong side of the move costs a mob about two blocks over that second while everything
still looks like falling. `src/lib/physics` is the arithmetic on its own, with those two numbers as
its test, and `src/bin/src/systems/physics` is the tick that drives it:

```
pull_before_moving  →  collisions  →  velocity  →  pull_and_slow_after_moving
```

Standing on something is decided each tick from whether the ground was still there to stop the
fall, not remembered — a block mined out from under an entity leaves it falling with no help.

### Stepping up

An entity that walks into a rise it could walk up tries the move again from a box lifted by its own
step height, and takes it only if it gets further along. A mob steps up 0.6; a dropped item steps
up nothing and walks into the slab.

## Regenerating

```bash
scripts/extract_entity_types.py   # asks the game what a type is; needs 26.1 or newer
scripts/build_entity_types.py     # writes the enum and its table

scripts/extract_synched_data.py 26.1   # asks the game how a row is laid out
scripts/extract_synched_data.py 26.2
scripts/build_synced_data.py           # writes the layouts, the field names and the vocabularies

scripts/extract_entity_physics.py      # asks the game how each type moves

scripts/extract_serializer_ids.py --all   # reads every version's kind numbers out of its jar
```

Both entity extractors build a real entity to ask it, which needs a world for it to be built in;
`scripts/extractor/GameEntities.java` is that world, answering the five things a constructor asks
and nothing else.
