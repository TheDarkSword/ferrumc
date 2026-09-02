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
