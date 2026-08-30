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
so a new wood type gains a door without anything in the code changing. Tags are read from the
loaded datapacks, so what a tag holds follows the packs rather than a table built into the binary;
see [Datapacks](../datapacks/README.md).

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

## Light

Light spreads outwards from what gives it off, losing at least one level per block and more through
anything that dims it. Taking a light away is the harder direction: everything it lit has to be
darkened first and then relit from whatever else still reaches it, because a level does not say
which of several sources it came from.

Two queues do that, as in vanilla: one carries light outwards, the other carries darkness, and
darkness is drained first. Both walk the six directions breadth-first, and an entry remembers which
directions it may still spread in so light never travels back the way it came.

What each state does to light — how much it gives off, how much it dims — comes from the extractor.
So does the awkward part: a slab dims nothing and still stops light through its flat side, because
some blocks occlude by shape rather than by opacity. Whether light passes between two blocks is a
question about both their faces together, which is a pair and cannot be tabulated, so each face's
own answer is, which settles every case but two partial faces that only cover the opening between
them.

Sky light works the other way round. Rather than one source dimming with depth, **every position
the sky reaches is a source at full strength** — which is what makes an open column bright all the
way to the ground — and the light then spreads sideways and under overhangs from those, losing a
level a block like any other. A column's lowest source is found by scanning down until something
stops the sky: anything that dims light at all does, and so does a pair of faces that closes the gap
between them, which is why a trapdoor lets the sky past when open and not when shut.

A generated chunk lights itself, both kinds, from what is in it. Because lighting must not be what
pulls a chunk's neighbours into memory, it is lit alone and the light is let across the borders both
ways when it is about to be sent. Vanilla instead lights a chunk with its neighbours' sources to
hand, as a stage of the chunk pipeline.

A block placed or broken relights what it changed and the result is sent, since clients do not work
light out for themselves — a torch placed after a chunk was sent would otherwise stay invisible.

Whether light passes between two blocks is a question about both their faces together: two partial
faces can cover the opening while neither covers it alone, which a top slab beside a bottom slab
does. There are only 55 distinct faces in the game, so every pair's answer is worked out once in the
extractor and looked up. 300 of those pairs stop light only together.

## Block entities

A chest's contents, a sign's text, a furnace's progress: none of it fits in a state id, so those
blocks carry a block entity alongside. 186 of the 1196 blocks do, and which block carries which
comes from the game itself — the set of blocks a type accepts is private, but the question "does
this type accept this state" is not, so it is asked directly.

They live in the chunk and are written with it, so what a sign says survives a restart. Placing a
block that carries one creates it and breaking it takes it away, both from `set_block`, so nothing
has to remember to. Replacing a chest with another chest keeps what was in it — which is what
happens when one is waterlogged or turned — while replacing it with anything else does not.

A sign's line is a **text component**, as it is in the game: it can be coloured, translated or carry
a click event, none of which a bare string says. What is stored is the component written out, which
is the same split vanilla has between the component it works with and the codec it saves through.

A kind that is not modelled yet still exists and is still written and sent; it simply carries
nothing. That way a client is told the block entity is there, and the day it gains fields nothing
else has to change.

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

## How much attention a chunk gets

A chunk is not simply loaded or not. Every chunk has a *level*, a number saying how close it is to
something that cares about it, and what the server does with it follows from that number:

| Level | What happens |
|---|---|
| ≤ 31 | Its entities tick |
| 32 | Its blocks tick: crops grow, fluids move, scheduled ticks run |
| 33 | Kept and sendable, but nothing in it happens |
| above | Not kept |

Levels come from *tickets*. Something that wants a chunk kept asks for it at a level, and the level
spreads outwards a step at a time, so one ticket keeps a whole neighbourhood at progressively less
attention. A player holds two: one for what they can see, and a tighter one for what goes on around
them, at `simulation_distance` in the config.

Those are two separate questions, so they are two separate sets of levels, as they are in vanilla.
One set would make a chunk near a player tick because the player can see far, which is not what a
simulation distance is for.

A scheduled tick due in a chunk that is not simulated waits rather than running, and a chunk that is
let go takes its waiting turns back and is written with them, so they are still due when it returns.
What is saved is the remaining wait rather than a tick number.

A stored chunk carries a format version. The encoding is not self-describing, so a chunk written by
an older layout would otherwise be read as whatever the current one expects and come back quietly
wrong; instead it is treated as absent and generated again.
