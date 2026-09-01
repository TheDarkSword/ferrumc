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

## How a mob comes to exist

Every tick, for every chunk a player is keeping loaded, the server tries to put a mob somewhere.
Almost every attempt fails, and that is the point: a world feels populated rather than crowded
because the questions are hard to pass.

`src/lib/spawning` holds the questions and knows nothing about the world — what the world has to
say is a `SpawnWorld`, and what comes back is a list of mobs to put down. `systems/mobs/natural.rs`
answers the questions from the chunks and turns the answers into entities.

The order is vanilla's, cheapest question first:

1. is there room for another of this group, in the world and in these chunks;
2. is a player at least twenty-four blocks away, and is anyone there at all;
3. does the biome have anything of this group to offer;
4. may this kind stand here — on the ground, in water, in lava, anywhere;
5. does its own rule hold — dark enough for a monster, light enough for an animal;
6. does it fit.

### Two caps, and a mob has to be under both

One counts every mob of a group in the world against how much of the world is loaded, which is why
one player alone sees far fewer mobs than a busy server does. The other counts them in the chunks
around a place, which is why a crowd in one valley stops more appearing beside it.

### What decides whether a place will do

Where a kind may stand is on the type, extracted with everything else. The condition beyond that is
a method reference in vanilla — one per type — so `scripts/extract_spawn_rules.py` reads which one
each type points at. Seven conditions cover fifty types; the other thirty-three belong to a single
mob each and are refused until that mob exists, which keeps a slime from appearing everywhere
rather than nowhere.

Whether a block may be stood on is the block's own answer, extracted per state: the default is a
sturdy top face giving off little light, and a few blocks differ.

## Who is told about what

A client is not told about everything. It is told about what is near it, at a range the entity's
own kind decides — ten chunks for most things, and the game's own number for each — measured along
the ground only, so something far below a player is still near it. Come into range and the client
is sent a spawn and the whole row it reads the entity by; leave and it is told to forget it.

How often it is told again is also the kind's own number: an arrow every tick, a painting never.

### Why the position is not sent

Between one update and the next what goes out is the change — three sixteen-bit numbers in
sixteenths of a thousandth of a block rather than three doubles. That only works if both ends agree
on where the entity was, and they only agree if the change is worked out the way the wire carries
it: rounding the old and the new positions to the wire's precision and subtracting *those*.

Subtracting first and rounding after loses a fraction every round. Nothing looks wrong — the entity
simply drifts until it is standing somewhere it is not. There is a test that walks an entity a
thousand uneven steps and checks the client ends up holding it exactly where it is.

A round that sends only a turn must not move the position the next change is measured from, or
everything after it is measured from somewhere the client was never told about. An outright position
goes out when the change is too large to carry, when the entity lands or leaves the ground, and
every four hundred rounds regardless — a client that missed a packet has no other way back.

## What survives a restart

An entity belongs to the chunk it stands in and is written with it, in a table of its own rather
than beside the blocks — the same separation vanilla keeps, so that adding to one cannot cost the
other.

A chunk coming into view brings back what was standing in it, keeping the name each mob had. A
chunk nobody is near any more is written out and its entities let go of. Everything loaded is also
written on the same timer the world is, so a crash costs seconds rather than a session.

A chunk that has never been seen has nothing saved and has never been populated, so that is when it
is given the animals it is born with. It carries a mark saying so afterwards: without one, a restart
would hand every chunk a second herd.

What is written is where a mob is and how it is moving. What it is beyond that belongs to the mob,
and no mob has any of its own yet.

## What a broken block leaves behind

Breaking a block asks its own loot table what it drops, so a stone mined without a pickaxe leaves
nothing. What comes back is put on the ground as an ordinary entity: it falls, it is tracked, it is
written out with its chunk. What makes it a dropped thing rather than a mob is only that it waits
before anyone may take it, joins its neighbours, gives up after five minutes, and goes into whoever
walks over it.

Experience is the same shape with different arithmetic. An amount does not become one orb but a
handful, largest first, in the sizes the game gives it — 2477, 1237, 617, 307, 149, 73, 37, 17, 7,
3, 1 — which is why killing something scatters several. An orb is pulled towards whoever is nearest
within eight blocks, harder the closer it is, rather than waiting to be walked over.

Merging matters more than it sounds: a floor covered in cobblestone is otherwise a thousand
entities, each tracked to every client near it and each written out with its chunk.

## Regenerating

```bash
scripts/extract_entity_types.py   # asks the game what a type is; needs 26.1 or newer
scripts/build_entity_types.py     # writes the enum and its table

scripts/extract_synched_data.py 26.1   # asks the game how a row is laid out
scripts/extract_synched_data.py 26.2
scripts/build_synced_data.py           # writes the layouts, the field names and the vocabularies

scripts/extract_entity_physics.py      # asks the game how each type moves

scripts/extract_serializer_ids.py --all   # reads every version's kind numbers out of its jar
scripts/extract_field_layouts.py --all    # and where each field sits in it
scripts/extract_spawn_rules.py            # which condition decides where a mob may appear
```

Both entity extractors build a real entity to ask it, which needs a world for it to be built in;
`scripts/extractor/GameEntities.java` is that world, answering the five things a constructor asks
and nothing else.
