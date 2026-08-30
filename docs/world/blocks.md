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

## Behaviour

What a block *does* is a table keyed on block, holding an entry only where there is behaviour to
hold. Vanilla dispatches this virtually across 326 block classes, most of which override nothing; a
block with no entry here costs a bounds check.

Blocks register by the vanilla tag that groups them — `#minecraft:doors`, `#minecraft:trapdoors` —
so a new wood type gains a door without anything in the code changing. Tags come from the version's
own data with their references to other tags resolved.

Method names follow `BlockBehaviour` in the vanilla sources so the two read side by side.
Implemented so far:

| Behaviour | Blocks |
|---|---|
| `use_without_item` | doors, trapdoors, fence gates |
| `update_shape` | doors |

A door's two halves stay in step the way vanilla does it: the other half takes on the state of the
one that changed and keeps only its own `half`. Setting `open` on both instead would work until
something else — facing, hinge, powered — needed to travel with it.

Iron doors and iron trapdoors do not open by hand. That is `BlockSetType.IRON` in the sources, and
the one set with `canOpenByHand` false among doors; copper opens.

Using a block comes before placing one, as in vanilla, so a door opens rather than a block being
placed against it. Vanilla skips that for a sneaking player holding something; whether a player is
sneaking is not tracked yet.

Not carried across yet: block sounds, game events, and the scheduled tick a waterlogged trapdoor
asks for. Random and scheduled ticks have hooks and nothing driving them until the tick system.

## Ticking

Two independent systems, as in vanilla.

**Scheduled ticks** are the ones a block asks for. Their order within a game tick is observable — it
is why redstone reads as it does — so they run by priority, then by which was asked for first,
across the whole world rather than per chunk. Grouping by chunk first would make the order depend
on a hash map's iteration.

A block has at most one pending tick per kind, whatever tick a second one would be due on. That is
vanilla's rule and it is what stops a block whose neighbours all update at once from queueing one
tick per neighbour.

Fluids and blocks are drained separately, as they are in vanilla, so a large fluid cascade cannot
eat a redstone tick's budget.

**Random ticks** are handed out rather than asked for: every section holding anything worth ticking
gets `random_tick_speed` positions a tick, three by default. Positions come from the same counter
vanilla uses — one multiply and add each — and a section with nothing that ticks is skipped without
being sampled. 1508 of 32366 states take a random tick, and it is a property of the state rather
than the block: a fully grown crop stops.

Sugar cane grows on this path, which is vanilla's simplest complete random tick: it needs no light,
no neighbours and no randomness of its own.

**Persistence** has its saved form and its take/restore API, and is not wired to disk yet. Where a
chunk's ticks live is decided by the chunk lifecycle work, and wiring it before that would mean
doing it twice. What is saved is the remaining wait rather than the tick number, so a world that
stops and starts again resumes instead of firing everything at once.

## Neighbour updates

Placing or breaking a block sets off a chain: its neighbours are told, they may change, and theirs
are told in turn. The order is observable — most of redstone's character comes from it — and there
are two of them, which are not the same:

| | Order |
|---|---|
| Neighbours are *told* | west, east, down, up, north, south |
| Neighbours *recompute their own state* | west, east, north, south, down, up |

The chain is walked with a stack rather than by recursion. Vanilla has both and uses the queued one
on the server, because a large contraption is deep enough to exhaust the stack.

Every update counts against the chain limit, including the ones an update produces while running.
Counting only what starts a chain would never catch the case the limit is for, which is a chain that
feeds itself.

Whether a block can stay where it is asks whether the face below holds it up. That is a question
about a block's support shape, and the extractor answers it directly — one bit per face and support
type — rather than carrying a fourth shape per state and the boolean algebra to slice it. A torch
sits on a fence post, whose face is not full but holds a centre; it does not sit on a bottom slab,
whose top is half a block below the face.

## Regenerating

```bash
scripts/extract_block_shapes.py     # asks the game for shapes; needs 26.1 or newer
scripts/build_block_states.py       # property tables and blockstates.json
scripts/build_block_shapes.py       # shape tables
scripts/build_block_tags.py         # block tags, references resolved
scripts/sync_data_assets.py --check # reports anything left behind by a version bump
```

Every one of these reads the version's own extracted data. A file left behind from an older version
is a wrong answer rather than a missing feature, and a silent one — see
[Known gaps](../networking/known-gaps.md) for what that cost once already.
