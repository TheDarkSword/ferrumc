# Blocks

A block state id is a number on the wire and in storage, and nothing else. What that number means —
which block it is, what properties it holds, what space it occupies — comes from tables generated
from the version the server speaks.

## State ids

The states of one block are the cartesian product of its property values, laid out contiguously,
and the blocks partition the id space in order with no gaps. Every question is therefore arithmetic
rather than a lookup:

| Question | How it is answered |
|---|---|
| Which block is this state? | Binary search over the block bases |
| What does it hold for a property? | Divide the offset by that property's stride |
| The same block with one property changed? | Subtract the old value, add the new |

The order the properties are combined in is **not** the order the vanilla report lists them — for
twelve blocks, the chests and the pistons among them, the two disagree — so
`scripts/build_block_states.py` measures each property's stride from the states themselves and
checks every state lands where its properties put it before writing anything.

## Properties

A property is read and written by naming it, and naming it names its type:

```rust
state.get(properties::FACING)                     // Option<Direction>
state.with(properties::FACING, Direction::North)  // Option<BlockStateId>
state.with(properties::POWER, 9)                  // integers are u8
state.with(properties::WATERLOGGED, true)         // flags are bool
```

The same name means different things on different blocks — a stair's `half` is its top or bottom, a
door's is its upper or lower; `type` belongs to slabs, chests and pistons alike — so each constant
pairs a name with the type it carries there. Asking a block for a property of the wrong type finds
nothing, the same as asking for one it does not have. `get_raw` and `with_raw` work in strings, for
the places that only have a name, such as commands.

Values that a version does not accept are refused rather than applied: a stair does not face up.

## Shapes

A block is not a cube. A slab collides at half height, a fence collides half a block taller than it
draws so that it cannot be jumped, and a carpet stops nothing.

Collision shapes are built in code, sometimes from lambdas over a block's own property values, so no
data report carries them and the game has to be asked for them. From 26.1 the server jar ships with
its own names, so `scripts/extract_block_shapes.py` compiles a small Java program straight against
it — no remapping, no mod.

A shape is a list of boxes rather than the bitmap over per-axis coordinates vanilla keeps: states
average 1.85 boxes and reach fifteen at the worst, and at that size the bitmap costs more than it
saves. 32366 states share 915 distinct shapes built from 683 distinct boxes.

Union, intersection and difference are not implemented. Nothing needs them yet; collision resolves
one axis at a time against each box in turn, which needs no boolean algebra. Occlusion, when the
lighting engine wants it, is what will.

## Collision

Movement is shortened before it is applied, rather than the entity being pushed back out
afterwards. Axes are resolved one at a time with the box carried forward between them, so an entity
walking into a wall keeps the speed it had along the wall. The vertical goes first — standing on
something has to be decided before sliding along it — then the larger horizontal.

Whether a block is in the way is its collision shape's business. The previous rule listed air, void
air and water by name and treated everything else as a full cube, which made torches, carpets and
flowers solid.

## Regenerating

```bash
scripts/extract_block_shapes.py     # asks the game for shapes; needs 26.1 or newer
scripts/build_block_states.py       # property tables and blockstates.json
scripts/build_block_shapes.py       # shape tables
scripts/sync_data_assets.py --check # reports anything left behind by a version bump
```

Every one of these reads the version's own extracted data. A file left behind from an older version
is a wrong answer rather than a missing feature, and a silent one — see
[Known gaps](../networking/known-gaps.md) for what that cost once already.
