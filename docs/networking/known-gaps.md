# Known gaps in multi-version support

All ten supported versions — **1.21 through 26.2** — join and play with no translation errors.
Verify with `scripts/check_versions.sh`, which runs each of them through ViaProxy and reports the
join count the bot observed.

What follows is what is known to be missing rather than broken. None of it stops a client
connecting.

## Serverbound bodies are translated one packet at a time

The clientbound direction has a translator for every body that differs. The serverbound direction
has one, `client_information`, because that is the only difference reached so far by a client
joining and moving.

ViaVersion's upgrade protocols name the rest. Within this range the bodies that change, per
boundary, are:

| Boundary | Serverbound packets whose body changed |
|---|---|
| 1.21 → 1.21.2 | `accept_teleportation`, `client_information`, `container_close`, `move_player_pos`, `move_player_pos_rot`, `move_player_rot`, `move_player_status_only`, `place_recipe`, `player_input`, `pong`, `recipe_book_seen_recipe`, `use_item_on` |
| 1.21.2 → 1.21.4 | `move_vehicle`, `pick_item_from_block`, `pick_item_from_entity` |
| 1.21.4 → 1.21.5 | `chat`, `chat_command_signed` |
| 1.21.5 → 1.21.6 | `change_difficulty`, `change_game_mode`, `player_command`, `player_input` |
| 1.21.7 → 1.21.9 | `debug_sample_subscription` |
| 1.21.9 → 1.21.11 | `client_tick_end`, `player_action` |

An older client sending one of these gets a decode error and is dropped. The work is one hop
function each, in the same modules as the clientbound hops.

## A teleport carries no velocity to 1.21

1.21.2 added a velocity to the play teleport. 1.21 has no field for it, and vanilla clients on that
version are pushed by a separate motion packet sent alongside. That packet is not sent yet, so a
teleport that would have carried a push arrives on 1.21 as a plain move.

Only teleports that set a velocity are affected, which the server does not currently produce.

## Registry field types are not per-registry

Registry entries are sent as NBT built from each version's datapack, and the tag a field gets is
inferred from its JSON value rather than from what the client expects. Strict clients log a missing
field for entries where the two disagree — `minecraft:enchantment` is the one that appears in
practice.

Inferring by value is right for most registries. Fixing this properly means carrying the field types
from the vanilla codecs, which is Phase 3 work.

## Ids are remapped for block states only

Block state ids are translated per version. Item, entity type, sound and particle ids are not, so a
packet carrying one sends the 26.2 id to every client. ViaVersion logs these as missing items when
it sees them.

Nothing sends those ids yet beyond what a join needs, which is why this has not surfaced as a
failure.
