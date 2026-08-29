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

## A spectator's attack is an attack

26.1 split the interaction packet: an attack became its own packet, and a spectator's attack became
a request to spectate that entity. Spectator mode is not tracked on the connection, so an attack
from an older client stays an attack.

## Packets no version difference has been written for

A packet is only translated once something sends or receives it. These have a difference somewhere
in the supported range and no translator, because nothing uses them yet:

| Direction | Packets |
|---|---|
| Clientbound | `container_set_data`, `horse_screen_open`, `open_sign_editor`, `place_ghost_recipe`, `player_rotation`, `recipe_book_add`, `recipe_book_remove`, `recipe_book_settings`, `set_cursor_item`, `set_passengers`, `update_recipes`, `chunks_biomes`, `change_difficulty`, `clear_dialog`, `server_links`, `show_dialog`, `initialize_border`, `set_border_center`, `set_border_lerp_size`, `set_entity_motion`, `mount_screen_open`, `remove_mob_effect`, `update_mob_effect` |
| Serverbound | `move_player_status_only`, `move_vehicle`, `place_recipe`, `recipe_book_seen_recipe`, `pick_item_from_entity`, `chat_command_signed`, `change_difficulty`, `rename_item`, `debug_sample_subscription`, `spectate_entity` |

Each is one hop function in the module for the boundary that changed it, written when the packet
itself is.

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
