# Item stacks and components

A stack is a count, a kind, and a **patch** of components over what that kind already says. A plain
diamond sword carries no components at all; a named one carries only the name.

## The thing that makes this delicate

A component carries **no length**. A reader that does not know a component's shape cannot skip it —
it has to read the payload or give up, because everything after it would be read at the wrong
offset.

So an unknown component is refused loudly rather than stepped over. A loud failure beats a
container read as nonsense.

What this replaced read the component **ids and not their payloads**, which meant any stack carrying
a single component desynced the rest of the packet.

## Shapes

Fifteen of the hundred and eleven have a known shape, and far fewer shapes than that:

| shape | components |
|---|---|
| `Nothing` | `unbreakable`, `creative_slot_lock` |
| `Number` (VarInt) | `damage`, `max_damage`, `max_stack_size`, `repair_cost`, `rarity`, `map_id`, `ominous_bottle_amplifier` |
| `Flag` | `enchantment_glint_override` |
| `Colour` (i32) | `dyed_color`, `map_color` |
| `Text` | `custom_name`, `item_name` |
| `Lines` | `lore` |
| `Enchantments` | `enchantments`, `stored_enchantments` |
| `Nbt` | `custom_data`, `block_entity_data`, `bees` |

Text is kept as **the NBT bytes it travels as** rather than parsed. A custom name is written by a
client and sent back to clients; nothing here needs to look inside one, and keeping the bytes is
lossless without a reader for every shape a text component can take.

Anything of the `Nbt` shape is likewise kept as it arrived, so it survives a round trip without
being understood.

## Two registries that move

Both a component type and an enchantment travel as a place in the **reader's own** registry, and
both have grown:

- component types: 57 → 96 → 110 → 111 across the supported versions; `custom_name` moved from 5 to 6
- enchantments: 42 → 43; `lunge` arrived in 26.1 in the middle of the alphabet and moved 21 of them

Both are looked up per version from that version's own report. One a client has never heard of is
**left out** and the count written after the leaving out.

## Reading one NBT value out of a stream

`ferrumc_nbt::streaming::read_one` walks a tag's shape and stops exactly where it ends. The slice
parser this crate is built on reads a whole document, which is right for a packet whose last field
is NBT and useless for components, which sit one after another.

It refuses a tag that does not exist, a negative length, and anything nested past 512 deep — a
stream is not to be trusted.

## Both directions, and why they are not the same table

An item travels as a place in the **reader's own** registry. This server keeps its own number and
translates only at the moment of writing to one particular connection, so:

- **Out**: a 26.2-only item reaches a 1.21 client as a stand-in from the same family, found by the
  longest shared trailing words — or as air where there is nothing close. The server's own number is
  untouched, so a 26.2 client looking at the same chest sees the real thing.
- **Placing**: the block placed comes from the server's own inventory, not from the number the
  client sent. A 1.21 client placing a 26.2-only block places the real block, and everyone with a
  client that has it sees it correctly.
- **In**: a client names an item by *its own* number. `container_click` and `set_creative_mode_slot`
  translate it back before storing it. Taking one at face value stores a different item — which is
  what happened before this was written.

Nothing is ever dropped from the server's own copy. An enchantment or a component a client cannot
see is left out of **that client's bytes** and nowhere else, so a `lunge`-enchanted sword shown to a
1.21 client is still a `lunge`-enchanted sword, and a 26.2 client looking at the same slot sees it.

The click path is where that could have been undone. A client's click packet carries components as
hashes, not values, so there is nothing in it to rebuild a name from — and rebuilding a slot from
the packet alone would quietly strip every stack a client ever moves. Instead, what was on the slots
the click names is read **before anything is written** and carried across to whatever arrives,
matched by kind and handed out once each.

The way back is **not the forward table reversed**. A stand-in is several names pointing at one
number, and reversing one of those would be a guess; the reverse table is built from the names, so
only an exact match comes back. A client can only ever name something its own version has, so
nothing is lost — and a number that names nothing is refused rather than acted on.

# Mining

`ferrumc-mining` holds the arithmetic; `systems/listeners/digging_system.rs` asks the world.

```
speed = tool_speed
      + mining_efficiency          only if the tool beats a bare hand
      × (1 + haste × 0.2)
      × [0.3, 0.09, 0.0027, 0.00081][fatigue]
      × block_break_speed
      × submerged_speed            if the digger's head is under water
      ÷ 5                          if there is nothing underfoot

progress a tick = speed ÷ hardness ÷ (30 if the tool is right, else 100)
```

Two things people do not expect:

- The **wrong tool** does not only stop the drop. The divisor goes from 30 to 100, so the block
  takes more than three times as long.
- **Mid-air** is five times slower and **underwater** five times again, and they stack — twenty-five
  times slower for a block broken while swimming upward.

Efficiency is added only where the tool already beats a bare hand, which is why efficiency on a hoe
against stone changes nothing.

A block that needs no particular tool is always "right", however it is being hit — so dirt takes no
*penalty* for a fist. That is not the same as saying nothing is faster: a shovel is worth eight
against dirt and a fist is worth one, so the shovel is still seven and a half times quicker. Dirt is
fifteen ticks by hand and two with a diamond shovel, and it comes up either way.

Which blocks come up bare-handed is the **block's** answer, not the tool's: dirt, grass, sand and
gravel say no tool is needed; stone and ore say one is.

## Where the numbers come from

Hardness and the right-tool flag are **per block state**, not per block — a lit furnace and an unlit
one are two states and need not agree. Neither is in any report; both live on the block's behaviour,
set in code. `scripts/extract_block_properties.py` asks the game for all 32366 of them.

The table is packed into five bytes a state and `include_bytes!`d rather than emitted as a literal
array: thirty-two thousand numbers is a token each, and minutes of compile time for something only
ever read.

Which blocks a tool's rule names is a tag the packs define, so it is asked of them. The **first**
rule that names the block wins, so their order is the tool's own answer and not something to sort.

## Wrong tool, no drops

Not a rule here: the block's loot table says so, through a `match_tool` condition. The break event
carries what was in hand and the loot context passes it on, so stone mined with a fist matches
nothing and leaves nothing.

# Putting things away

`Inventory::add_item` takes as much of a stack as fits and hands back whatever would not, so a
player walking over a stack with one slot free takes what that slot holds and leaves the rest on the
ground rather than losing it.

## The order vanilla looks in

**To top up** an existing stack: what is in hand first, then the off hand, then the hotbar, then the
main store. That is what makes mining feel right — the stack being held grows.

**To place** a new stack: the same order **less the off hand**. Vanilla will top up what is already
held there and will never put something new into it, which is why a picked-up block does not land in
the shield hand.

The armour slots and the crafting grid are in neither list.

## What counts as the same thing

The kind **and** everything the stack says about itself. A named sword does not merge into a plain
one; two swords with different damage are two stacks. That falls out of comparing the whole
component patch, which is why it had to exist first.

## How much fits

The item's own `max_stack_size`, not a flat sixty-four: sixteen for pearls, one for a sword.

Anything that is not a player's inventory — a chest, once there is one — is a plain row of slots and
is walked in order.

# Crafting

## Taking the result is the one click the server carries out itself

A click packet says the grid emptied and the result appeared. Believing both halves is how one plank
becomes a crafting table **and** a plank again — the client is describing a trade, and a server that
writes down what it is told has performed only one side of it.

So when a click names the output slot and the output is actually set, the server spends the grid
itself: one of each ingredient, and then re-runs the match so holding the button keeps producing.
What the client says about the grid and the output for that click is ignored, because it is a report
of the same trade and would spend it twice.

## Remainders

A bucket of milk in a cake leaves the bucket. It goes back in the slot it came from where that slot
is now empty, and is handed back to the caller otherwise — a slot that still holds more milk has
nowhere to put it.

Five items leave something behind: the three buckets, dragon's breath and a honey bottle. That is a
field on the item rather than a component, and **not** the same as what drinking something leaves —
the two lists overlap and are not the same list. `scripts/extract_crafting_remainders.py` asks the
game.

The dump this replaced was five entries of an older version's ids, which decoded through the current
registry as "a diamond hoe leaves a diamond axe".

# Holding an item down

Eating, drinking and drawing a bow are the same shape: a right-click starts it, it counts down while
held, and something happens at the end. What a particular item does is on the item — a consumable
takes as long as its `consume_seconds` says, and anything without that component cannot be held down
at all.

Only eating and drinking are wired up. A bow and a trident are held the same way and each needs a
projectile at the end of it.

## What the client draws

Two flags in the entity's data: one says something is being used, the other says which hand. Both go
out when the use starts and come off when it stops — a client that is never told it stopped goes on
animating an item that was finished with.

## What stops a use

Putting the item down. Without that check, swapping to a sword mid-meal would still finish the meal,
because the countdown does not care what is in the hand.

## What is left in the hand

Drinking a potion leaves the bottle. It takes the slot where the slot is now empty, and goes
elsewhere in the inventory where more of the drink is still there.

That is the `use_remainder` component — **not** the crafting remainder. The two lists overlap and
are not the same: seven items leave something behind on being used, five on being crafted with.
