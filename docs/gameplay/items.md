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

The way back is **not the forward table reversed**. A stand-in is several names pointing at one
number, and reversing one of those would be a guess; the reverse table is built from the names, so
only an exact match comes back. A client can only ever name something its own version has, so
nothing is lost — and a number that names nothing is refused rather than acted on.
