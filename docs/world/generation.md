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

## Density functions

Terrain shape is not a formula, it is a **tree**: noise, constants, arithmetic, clamps, splines and
interpolation, composed in the packs and evaluated at a position. Changing where mountains go is
changing the data, not the code.

Thirty-five functions across five dimensions, read from
`data/minecraft/worldgen/density_function/`. A function that names another is built after it,
however the two are ordered on disk, and a pack that names itself in a circle costs that one
function rather than the stack.

### The spline is the part that has to be exact

It is where continentalness and erosion become landforms. Between two points the curve is a cubic
fitted to the value **and the slope** at each end:

```
a = d₁·(x₂-x₁) - (y₂-y₁)
b = -d₂·(x₂-x₁) + (y₂-y₁)
lerp(t, y₁, y₂) + t·(1-t)·lerp(t, a, b)
```

Past either end it carries on as a straight line at the slope it was leaving with. A straight line
through the same points gives terrain that reads as *wrong* rather than as different, which is why
this one piece is written out exactly even though seed-for-seed exactness is not the aim.

A point's value is itself a spline, which is how erosion bends what continentalness said.

### Two details that decide how a coast looks

- `half_negative` and `quarter_negative` touch only what is **below** nothing, which flattens
  valleys while leaving hills alone.
- A range choice takes its bottom bound and not its top, so a value exactly on the edge falls
  inside.

### What is transparent, and what is nothing

The caching wrappers evaluate their inner function in full. The answer is right; the cost is not.
Seven kinds read as nothing rather than being guessed at: the four blending functions belong to a
world being extended from an older one, and the rest to dimensions that do not exist yet.

See `internal_docs/deferred.md` under Phase 6.2.

## Which biome goes where

Not a map and not a set of rules: a **nearest-neighbour lookup in six dimensions** — temperature,
humidity, continentalness, erosion, depth, weirdness. Each biome claims one or more boxes in that
space, and a place gets whichever box is nearest to where its climate falls.

Nothing owns a region of the world. A biome owns a region of *climate*, and the terrain decides
which climates turn up where. That is why biomes border plausibly without anyone saying they
should: two biomes that neighbour on the map are two boxes that neighbour in climate, and the smooth
noise that produces the climate cannot jump between distant ones.

The overworld has **7594 claims across 55 biomes**; the nether has five biomes.

The distance to a box is **nothing when the point is inside it**, so the middle of a biome matches
outright and only the edges are contested. `offset` is a flat penalty added to every distance — it
is not an axis, nothing is ever measured along it, and it is how a biome is made to lose a tie it
would otherwise win.

### The search has to skip most of the list

Seven and a half thousand claims, asked around a thousand times a chunk, is not something to walk.
The claims are held in a tree of boxes: each branch knows the smallest distance anything under it
could be, and a branch that cannot beat what has already been found is skipped whole. The nearer
half is walked first, so the further one is likelier to be skipped.

It looks at **under a twentieth** of the claims on average.

That is asserted as a count rather than a duration: a clock in a test suite that runs in parallel
measures the machine. And a separate test checks the tree gives the same answer as looking at all
7594 for three thousand random climates — a faster way to be wrong is not an improvement.

A tie is broken by the order the report lists claims in, whichever branch was walked first.
