# Known gaps in multi-version support

All ten supported versions — **1.21 through 26.2** — join and play with no translation errors.
Verify with `scripts/check_versions.sh`, which runs each of them through ViaProxy and reports the
join count the bot observed.

What follows is what is known to be missing rather than broken. None of it stops a client
connecting.

## Item stacks are not translated between shapes

1.21.5 changed how an item's data components travel: a client sends a hash of them rather than the
components themselves. `container_click` and `set_creative_mode_slot` from a client older than that
carry the components, and are read as though they were hashes.

Nothing reads those components yet, so the mismatch has no effect beyond the two packets being
wrong. Fixing it means implementing the hashing both ways, which belongs with the inventory work in
Phase 5.

## The chunk biome packet builds its own payload

`chunks_biomes` carries each chunk's biome containers as opaque bytes, and those containers changed
shape at 1.21.5 along with the ones in a chunk packet. Whatever builds the payload has to encode it
for the reader's version, the way the chunk packet already does; there is nothing for a hop to do
once it has.

## A spectator's attack is an attack

26.1 split the interaction packet: an attack became its own packet, and a spectator's attack became
a request to spectate that entity. Spectator mode is not tracked on the connection, so an attack
from an older client stays an attack.

## Packets with no struct yet

A packet is only written once something sends or receives it. These have a difference somewhere in
the supported range and neither a struct nor a hop, because they carry data models this server does
not have:

| Packet | What it needs first |
|---|---|
| `update_recipes`, `recipe_book_add`, `recipe_book_remove`, `recipe_book_settings`, `place_ghost_recipe` | the recipe display tree |
| `show_dialog` | the dialog tree |

Two more exist only in older versions and have no 26.2 counterpart to write: `spectate_entity`,
which 26.1 folded into the attack, and `debug_sample_subscription`.

## A teleport carries no velocity to 1.21

1.21.2 added a velocity to the play teleport. 1.21 has no field for it, and vanilla clients on that
version are pushed by a separate motion packet sent alongside. That packet is not sent yet, so a
teleport that would have carried a push arrives on 1.21 as a plain move.

Only teleports that set a velocity are affected, which the server does not currently produce.

## Items with no counterpart become air

An item a version does not have takes a stand-in from the same family, matched on the last word of
its name: a `copper_pickaxe` becomes an `iron_pickaxe`. Names whose family sits at the front instead
— a `music_disc_lava_chicken` is a disc, not a chicken — find nothing and become air, which reads as
an empty slot.

Vanilla proxies show a placeholder carrying the original name instead, which needs the item's
components rewritten as well.

## Registry field types are not per-registry

Registry entries are sent as NBT built from each version's datapack, and the tag a field gets is
inferred from its JSON value rather than from what the client expects. Strict clients log a missing
field for entries where the two disagree — `minecraft:enchantment` is the one that appears in
practice.

Inferring by value is right for most registries. Fixing this properly means carrying the field types
from the vanilla codecs, which is Phase 3 work.

## Tag ids are the server's own

`update_tags` carries bare registry indices, and those shift between releases. Item, entity, sound,
particle and effect ids sent elsewhere are translated for the client's version; this packet is built
once and sent as-is, so a client on 1.21 is told the current version's index of every item and block
in every tag.

What that costs a player is client-side and mostly cosmetic — the creative search and the recipe
book group items by tag — but it is wrong for every version but the newest. Fixing it needs a block
table alongside the ones the remapper already carries, and the packet built per connection rather
than once.
