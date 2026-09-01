# Extracting vanilla data

The registries, packet ids, block states and datapack contents the build consumes all come from
the official server jar, extracted by its own data generator. Two scripts cover the pipeline, and
both are reproducible from a clean checkout.

## `scripts/extract_assets.py`

```bash
scripts/extract_assets.py 26.2          # writes assets/extracted/26.2/
scripts/extract_assets.py --list        # released versions Mojang publishes
```

Resolves the version against Mojang's manifest, downloads the server jar, verifies it against the
published SHA-1, then runs the jar's data generator (`--server --reports`). Output:

| Path | Contents |
|---|---|
| `reports/packets.json` | packet ids per state and direction |
| `reports/blocks.json` | every block state and its properties |
| `reports/registries.json` | numeric ids for the static registries |
| `reports/commands.json` | the brigadier command tree |
| `reports/biome_parameters/` | the multi-noise biome placement tree |
| `data/minecraft/` | the built-in datapack: loot tables, recipes, advancements, tags, worldgen |
| `version.json` | protocol number and world data version |

Jars are cached under `~/.cache/ferrumc-mc-jars`. Output is kept per version so several releases
can sit side by side, which the multi-version protocol work needs.

Running the generator needs a JDK matching the release — 26.x needs JDK 25 or newer.

## `scripts/build_registry_packets.py`

```bash
scripts/build_registry_packets.py assets/extracted/26.2
```

Builds `assets/data/registry_packets.json`, the contents of every registry the server sends during
the configuration state, from the extracted datapack.

Two things it has to get right:

- **Which registries are synchronized.** The list is in the script and comes from
  `RegistryDataLoader.SYNCHRONIZED_REGISTRIES`. It grows between releases; 26.2 sends 29.
- **Which fields go on the wire.** Several registries keep more on disk than they send. Biomes drop
  their generation and spawn settings, and every animal variant drops its spawn conditions. The
  script carries that list, derived from the gap between each type's `DIRECT_CODEC` and its
  `NETWORK_CODEC`.

Entries are written sorted by name, because their order on the wire defines their numeric ids.

## Verifying a change to the pipeline

Regenerate an older version and compare against what the repository already had. Rebuilding
`registry_packets.json` from a 1.21.8 extraction reproduces 13 of the 15 registries committed for
that version byte for byte; the two that differ are entries missing from the older file, not
generator mistakes.

## Attributes

`scripts/extract_attributes.py` asks the game for every attribute: the number it travels as, what
it starts at, the range it is held to, and whether a client is told about it. None of that is in any
report — the number is, but the rest lives on the attribute object.

Output is `assets/extracted/attributes.json`, read at build time by `ferrumc-data`.

## Items

Item components are **not** extracted. The game's own `--reports` writes one file per item under
`reports/minecraft/components/item/`, and `ferrumc-data` reads those together with
`assets/data/registries.json` for the numbers.

Reading a dump beside the reports instead is how the table went 121 items short with 1389 of its
1416 numbers wrong, unnoticed, because nothing outside that crate reads them yet.
