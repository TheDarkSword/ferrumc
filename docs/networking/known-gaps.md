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

## Registry field types come from the game, for the two newest versions

Registry entries are sent as NBT built from each version's datapack. Json has one number type and
NBT has six, so a field's tag cannot be inferred from its value: most numeric fields in these
registries are floats, some are ints, and the same field name means different things at different
depths — an enchantment writes `base` as an `Int` in one place and a `Float` in another.

The tag of every field is therefore asked of the game itself: each entry is read through its own
codec and written back out as NBT, and the tag at every path is recorded.
`scripts/extract_registry_tags.py` does it, and only for 26.1 and newer, since older jars are
obfuscated and the extractor cannot compile against them.

Older versions use the 26.1 table. Of the 754 field paths 26.1 and 26.2 share, not one carries a
different tag, so the table is treated as version-stable; a field an older release had and 26.1 does
not keeps the earlier default, which is an `Int` for a whole number and a `Double` for a real.

## Tag ids are the server's own

`update_tags` carries bare registry indices, and those shift between releases. Item, entity, sound,
particle and effect ids sent elsewhere are translated for the client's version; this packet is built
once and sent as-is, so a client on 1.21 is told the current version's index of every item and block
in every tag.

What that costs a player is client-side and mostly cosmetic — the creative search and the recipe
book group items by tag — but it is wrong for every version but the newest. Fixing it needs a block
table alongside the ones the remapper already carries, and the packet built per connection rather
than once.

## Entities are spawned with a stale type id

The entity type sent in `spawn_entity` comes from a generated table that predates the 26.2 bump.
131 of its 151 ids disagree with the version's own registry: a type was inserted before `cat`, and
everything from there on is shifted by one or more. A pig therefore reaches the client as whatever
now sits at the old index.

The player is right, because that packet reads its id from `registries.json` rather than the table.

Fixing it means regenerating the table, which needs the extractor rather than the vanilla reports:
it carries health, dimensions and spawn rules that the reports do not have.

## An advancement's icon is not translated for 26.1

ViaVersion's `Protocol26_1To26_2` fails to remap `update_advancements`, in its item rewriter, and the
packet is dropped for a client on 26.1. Nothing else in the packet is affected and the connection
survives; the advancement screen simply has nothing in it.

26.2 writes an advancement's icon as an item *template* — the item's id, then the count, then the
component patch — where earlier versions wrote a slot, which leads with the count. The packet here
follows 26.2's own `DisplayInfo.serializeToNetwork`, and a 26.2 client, which needs no translation,
accepts it. The snapshot of ViaVersion the version check runs against reads it the older way.

There is nothing to fix on this side unless the reading turns out to be right, which would show as
a 26.2 client rejecting the packet too.
