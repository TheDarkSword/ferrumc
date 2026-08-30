# Datapacks

Everything the game defines in json — tags, loot tables, recipes, advancements, worldgen — is read
through one layer rather than baked into the binary. A version bump becomes a re-extraction, and
datapack support falls out of it instead of being added on top.

## What a pack is

A directory or a zip with a `pack.mcmeta` at its root and `data/<namespace>/<registry>/<path>.json`
underneath. The metadata says which versions of the game the pack was written for:

```json
{"pack": {"description": "My pack", "min_format": [107, 1], "max_format": [107]}}
```

A bare major in `max_format` means every minor release of it. The older `pack_format` and
`supported_formats` fields are still read, but they cannot name a version past 81, so a pack for a
current release has to use the pair. The format the server itself is — 107.1 for 26.2 — is read from
the extracted `version.json` when the crate is built, so it cannot drift from the data it describes.

A pack with no readable `pack.mcmeta` is not a pack, and is passed over with a line in the log.

## The stack

Packs stack. The one the server ships with is at the bottom; anything found in `datapacks/` is
layered on top of it in name order, and a pack that declares the same file as one below it wins.

Most things want only the winner — a loot table is one file. Tags want every copy, because a tag
merges across the packs that declare it rather than being replaced by the last one.

```
datapacks/
  mine/                     -> pack id "file/mine"
    pack.mcmeta
    data/minecraft/tags/block/logs.json
  theirs.zip                -> pack id "file/theirs.zip"
```

Every pack found is turned on, which is what vanilla does for a pack it has not seen before.
`/reload` looks at the directory again: packs that appeared are picked up, packs that are gone are
let go, and everything datapack-driven is read again.

## The pack in the executable

Vanilla keeps its own datapack inside the server jar. The same file is carried inside the FerrumC
executable: `src/lib/datapack/build.rs` builds a zip out of `assets/extracted/<version>/data`,
minifying the json on the way in, and the crate embeds it. It is opened through the same reader that
opens a player's zip, so there is one code path rather than a special case for the data the server
was built with.

It costs about 2.5 MB of binary and opens in a few milliseconds, since only the entries actually
asked for are decompressed.

The built-in feature packs vanilla ships alongside it — `trade_rebalance`, `minecart_improvements`,
`redstone_experiments` — are deliberately left out: vanilla offers those as separate packs gated on
feature flags, which do not exist here yet.

## Tags

A tag is a named set over a registry. A tag file lists elements by id and other tags by `#id`:

```json
{"values": ["#minecraft:logs_that_burn", "#minecraft:crimson_stems", "minecraft:sponge"]}
```

An entry may be written as `{"id": "...", "required": false}`, which is skipped when what it names is
not there. A *required* entry that is missing sinks the whole tag, and anything that referred to that
tag sinks with it — which is what vanilla does, and is worth knowing when a tag quietly disappears.

`{"replace": true}` drops everything the packs below declared instead of adding to it.

Tags are resolved once when the packs are read: references between them are followed in dependency
order and flattened, so a query never follows a reference. What a query sees is a bitset — one bit
per element of the registry — alongside the members in the order the tag declared them, which is
what vanilla's insertion-ordered set gives and what anything picking out of a tag by index needs.

A cycle between two required references loses both tags, again as vanilla does.

### What is wired up

Block tags are queried by the server through `ferrumc_world::block_tag`, and the `update_tags`
packet builds the block, item, fluid, entity type, game event and point-of-interest registries from
the same packs, so a pack that changes a tag changes what the client is told about it too.

Item tags still reach the recipe matcher through a table generated at build time; that goes when the
recipe work replaces the matcher.

## Predicates

A predicate is the condition language everything gates on: loot tables, advancements, and later
functions. It is one language in two halves in vanilla — `advancements/predicates` describes things
(a block, an item, a place), `loot/predicates` composes them into conditions — and one crate here,
`ferrumc-predicates`.

A condition names its type and reads whatever that type needs:

```json
{"condition": "minecraft:block_state_property", "block": "minecraft:wheat", "properties": {"age": "7"}}
```

They nest through `all_of`, `any_of` and `inverted`, and a bare list of them anywhere a condition is
expected means all of them. `reference` names a predicate from `data/<namespace>/predicate/`, which
vanilla ships none of — it exists for datapacks — and a reference that leads back to itself is caught
rather than followed.

### What it is asked against

A **loot context**: a bag of parameters saying what is going on, a source of randomness, and a way
to reach the world. A predicate wanting a parameter that is not there fails rather than erroring,
which is what makes `killed_by_player` mean "there was a player" at all, and what makes a position
that is not loaded fail a location check.

The world is reached through `LootWorld`, which names only what a predicate may ask: the block at a
position, the light there, whether it sees the sky, the dimension, the time, and the weather.

### What is not answered yet

Some conditions need something the server does not have. They are read, so a file carrying one still
loads, and they do not hold:

| Condition | Waiting on |
|---|---|
| `entity_properties`, `entity_scores` | entities, scoreboards |
| `damage_source_properties` | damage sources |
| `random_chance_with_enchanted_bonus` | enchantments on an entity |
| `environment_attribute_check` | environment attributes |

An item's components are the same story. `match_tool` asking whether a tool has silk touch — which
is most of its use in vanilla's own tables — answers no, and that is the right answer for a tool
with no enchantments, which is every tool today.

An unknown condition type is refused outright rather than treated as holding: a table that silently
stopped gating would change what it drops.

## Loot tables

Every drop in the game — a broken block, a killed mob, a chest, a fished item — comes from a loot
table. A table is a list of pools; a pool rolls a count and each roll draws one entry, weighted;
an entry produces item stacks; functions modify them on the way out. Conditions gate at every level.

```json
{"pools": [{"rolls": 1, "entries": [
  {"type": "minecraft:item", "name": "minecraft:redstone", "weight": 3},
  {"type": "minecraft:item", "name": "minecraft:emerald", "weight": 1}
]}]}
```

Functions run innermost first: the entry's own, then the pool's, then the table's, which is the
order that lets a pool cap what its entries produced.

`alternatives` takes the first child that can run and nothing after it — which is how a block says
"silk touch gives the block itself, otherwise this". `sequence` gives up at the first child that
cannot run; `group` takes them all. A `loot_table` entry rolls another table in place, and a table
that leads back to itself is caught rather than followed.

An entry or function type the game does not have is refused outright: a table that silently stopped
gating would change what it gives without saying so.

### What is not produced yet

A stack here is an item and a count. Vanilla's carries components too, and twenty-seven of the
game's forty-three functions set one — an enchantment, a name, a potion, damage. Those are read and
leave the stack alone, so a table that uses one drops the plain item rather than failing.

The three fortune formulas are written and correct, and read a level of nought, because nothing puts
an enchantment on a tool yet. An ore therefore drops its base count.

`slots` and `dynamic` entries read what a container or block entity holds, and produce nothing.

## Recipes

Every recipe is a file saying what goes in and what comes out. The shapes differ by type — a grid, a
bag of ingredients, a furnace, a stonecutter, a smithing table — and an ingredient is one item, a
list of them, or a tag.

A **shaped** recipe is a pattern of symbols with a key:

```json
{"type": "minecraft:crafting_shaped", "key": {"#": "#minecraft:planks"},
 "pattern": ["##", "##"], "result": {"id": "minecraft:crafting_table"}}
```

The grid is trimmed to the corner the items actually occupy before it is compared, which is what
lets a two-by-two shape be laid anywhere in a three-by-three grid. The pattern is tried as written
and mirrored left to right — **not** rotated: a recipe laid sideways does not craft, in the game or
here.

A **shapeless** recipe takes its ingredients in any arrangement. Matching them is an assignment
rather than a walk: two ingredients can each accept either of two items, and taking the first that
fits would strand the other.

**Cooking** recipes carry their experience and their time, and the default time depends on the
appliance — two hundred ticks in a furnace, a hundred in a blast furnace, a smoker or on a campfire.

### What does not craft yet

Sixteen types read and match nothing: every one of them reads or writes an item's components — a
firework's colours, a book's pages, a repaired tool's damage — which is why vanilla writes them as
code rather than data. A smithing trim is the same story.

Only the player's own two-by-two grid crafts. A crafting table, furnace, stonecutter and smithing
table all match correctly and have no screen to match in yet, and nothing tells the client which
recipes exist, so the recipe book stays empty.

## Advancements

An advancement is a set of named criteria, each a trigger with conditions, plus a rule saying which
of them together count as done:

```json
{"criteria": {"crafting_table": {"trigger": "minecraft:inventory_changed",
   "conditions": {"items": [{"items": "minecraft:crafting_table"}]}}},
 "requirements": [["crafting_table"]]}
```

`requirements` is an and of ors: every group needs one of its criteria granted. With nothing said,
each criterion is a group of its own, so all of them are needed.

Most of what the game ships is not shown at all: 1561 of the 1688 are the hidden ones that unlock a
recipe, which have no `display` and hang off a root that can never be earned.

### Where they sit on the screen

The client draws each advancement where the server says, so the layout is worked out when the packs
are read — the same tree layout vanilla uses, with depth along one axis and siblings along the other,
pushing subtrees apart where they would collide. An advancement with nothing to show takes no place,
and whatever hangs off it hangs off its parent instead.

### What a player has done

Kept per player in a table of its own, as vanilla keeps it in a file of its own, so that adding a
field to the rest of their data cannot cost them their advancements. It is read when they join and
written when they leave, along with when each criterion was earned.

### What fires

Three triggers: `impossible` (never, which is what the hidden roots use), `tick`, and
`inventory_changed`. The other fifty-two are read, so an advancement carrying one still loads and
shows on the screen, and they never fire — each waits on the gameplay that would fire it.

## Adding something datapack-driven

Read it through the stack rather than from a file path, and rebuild it where the rest is rebuilt:

```rust
// Every file in a directory, keyed by the id it holds, winner only.
let tables = FileToId::json("loot_table").list(&manager);

// Every pack's copy, for the things that merge.
let stacks = FileToId::json("tags/block").list_stacks(&manager);
```

`Datapacks::rebuild` in `src/bin/src/systems/datapacks.rs` is the one place that runs on both the
first load and a reload. A new consumer adds its line there and gets `/reload` for free.

## Worldgen definitions

Biomes, the features that go in them and the structures that are built are all datapack json, and
all of it is read here into types a generator runs. Reading them is this layer's job; running them
is worldgen's.

### The vocabulary is where the care goes

Everything is built out of a handful of shared value types, and three of them share type names while
differing in their fields:

| Written | Whole number | Real number | Height |
|---|---|---|---|
| `uniform` | `min_inclusive`, `max_inclusive` | `min_inclusive`, `max_exclusive` | two anchors |
| `trapezoid` | `min`, `max`, `plateau` | `min`, `max`, `plateau` | `min_inclusive`, `max_inclusive` |

So each is read where one is expected rather than guessed at from the type name. Two more that look
like what they are not: a rule-based block provider tests a **block predicate**, not a rule test, and
a set of ids written with one entry is a bare string rather than a list of one.

A block state is `{"Name": ..., "Properties": {...}}`, and a property the block does not have makes
the whole state unreadable rather than quietly giving the default — which would put the wrong block
in the world.

### What is read

Every one of the game's 226 configured features, 262 placements, 66 biomes, 34 structures and 20
structure sets. A feature type the game does not have is refused; twenty-seven that it does have are
recognised and their config kept as written, because nothing runs them yet and modelling a geode's
dozen fields before a generator asks for them would be guessing.

Noise settings, density functions and surface rules are not read here: they are the shape of the
terrain rather than what is placed on it, and they belong with the generator that runs them.
