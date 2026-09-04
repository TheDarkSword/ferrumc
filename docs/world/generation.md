# World generation

## Random sources

Two of them, because the game still carries both, and **both are copied exactly**.

The reason is not the terrain. A world here is meant to be *vanilla-like* — the same biomes in the
same sorts of places, not the same seed producing the same hill. But the same sources seed
structures, features and loot tables, and those have to be reproducible: two runs of the same server
on the same seed must lay the same chest down.

| | what it is | why it is still here |
|---|---|---|
| `Legacy` | `java.util.Random`, 48-bit LCG | a great deal of older generation was tuned around its exact output |
| `Xoroshiro` | Xoroshiro128++ | everything since the cave update |

Checked against the canonical values: `new Random(0).nextInt(100)` gives 60, 48, 29.

### Turning a place into a source

Nothing walks the world in order. A feature at one place has to derive its own randomness without
knowing what else has been generated, so a **positional factory** turns a coordinate — or a *name* —
into a source of its own.

The name matters more than it looks. Each octave seeds itself from `octave_<n>`, so an octave whose
amplitude is zero can be skipped **without moving every octave after it**. Seeding from a running
source instead would make the whole stack depend on which octaves happened to be silent.

### Two constants worth pointing at

- The state is never both zero. A Xoroshiro state of nothing produces nothing forever, so such a
  seed is replaced.
- A double is built by multiplying at **single** precision, then widening — as the game writes it.
  The obvious constant is a hair different, and a hair is enough for two implementations to disagree
  about which side of a threshold a piece of terrain falls on.

## Noise

Three layers, each built on the last:

1. **Perlin** — one lattice of gradients. Smooth, and far too smooth to be terrain.
2. **Layered** — several at doubling frequencies and halving weights, which gives a hill both its
   shape and its bumps.
3. **Noise** — two stacks laid over each other at a slight offset (×1.0181, so they do not rise and
   fall together), scaled so the result is roughly normal about nothing.

That last scaling matters as much as the noise itself: everything downstream compares against a
threshold, and a threshold means nothing unless the spread is known.

The gradient table has sixteen entries for twelve directions — four are repeats, a wart of the
original algorithm that everything since has been tuned around, so they stay.

## The octaves come from the packs

Sixty-one named sets, read from `data/minecraft/worldgen/noise/`. `continentalness` is octave -9
with nine amplitudes; `temperature` is -10 with six. A datapack that changes the shape of a world
changes it here too, without a rebuild.

## What is not here

See `internal_docs/deferred.md` under Phase 6.1. Chiefly: the old OpenSimplex pipeline still
generates the world. This is the foundation under the replacement, and what consumes it is the
density function tree in 6.2.
