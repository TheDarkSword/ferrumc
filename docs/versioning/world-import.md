# World import

`ferrumc import --import-path <world>` reads a vanilla world's `region/` directory and writes its
chunks into the server's own storage.

## What can be imported

Only worlds whose chunks carry the data version this server reads — currently **4903**
(Minecraft 26.2), defined as `SUPPORTED_DATA_VERSION` in `src/lib/world/src/importing.rs`.

Every chunk records the data version it was last written with. Chunks carrying anything else are
skipped rather than converted, and the import reports how many and which versions. If nothing at
all could be imported, the import fails with the version it found and the one it expected.

## Why other versions are rejected rather than converted

Minecraft's own chunk migrations live in `DataFixerUpper`, a chain of thousands of rules covering
every format change since 2011. Porting it is out of scope, and a partial port is worse than none:
a chunk that *almost* converts produces a world that is silently wrong — misplaced blocks, lost
block entities, biomes at the wrong resolution — rather than one that visibly fails.

Rejecting is also cheap to work around. Minecraft upgrades a world in place the first time it is
opened, so the path for any older world is:

1. Open the world once in Minecraft 26.2 and let it save.
2. Import it.

## Mixed-version worlds

A world played across several releases can hold chunks at different data versions, because a chunk
is only rewritten when it is visited. Such a world imports the chunks that match and skips the
rest, with a warning naming the versions it found. Opening it once in 26.2 and flying through the
affected area is enough to have Minecraft upgrade them.

## What import does not carry over

Only chunks. Player data, level settings, scoreboards, advancements, and the world seed beyond
what the generator needs are not imported.
