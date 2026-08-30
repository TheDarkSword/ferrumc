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
