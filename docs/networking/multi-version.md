# Speaking to more than one client version

The server holds one world and builds every packet in its own version — currently 26.2. Clients on
older versions are served by translating on the way out, and their own packets are translated back
up on the way in, so nothing above the network layer has to know which version a player is on.

Supported range: **1.21 through 26.2**, protocol 767 to 776. The lower bound is where Mojang's data
generator starts emitting a packet report; see [Target version](../versioning/target-version.md).

## The three things that vary

| What | Where it is handled |
|---|---|
| Packet ids | Generated tables, applied by the `NetEncode` derive |
| Registry contents | One payload per version, chosen when the client logs in |
| Packet bodies | Per-hop translator modules |

### Ids

Every packet's id in every supported version is baked in from the extracted reports, so the derive
writes the right one and no translator has to think about it. A packet a version does not define is
an error at encode time rather than a wrong id on the wire.

Serverbound ids are matched the same way: the play dispatch matches on version and id together, and
anything that waits for a specific packet resolves it with `lookup_packet_versioned!`.

### Registries

The set of synchronized registries and their contents both change between releases — 1.21 sends 11,
26.2 sends 29 — so each version has its own payload, built from that version's own datapack by
`scripts/build_registry_packets.py`.

### Bodies

Packets whose fields changed point at a translator with `#[downgrade_with(..)]`:

```rust
#[derive(NetEncode)]
#[packet(packet_id = "login_finished", state = "login")]
#[downgrade_with(crate::translate::to_26_1::login_finished)]
pub struct LoginSuccessPacket<'a> { .. }
```

The derive writes the id, then hands the body to that function. It returns `None` when the client
is new enough to read the native form, so the common case costs one comparison.

Translators live in `src/lib/net/src/translate/`, one module per release boundary, named for the
version it serves. Minecraft changes between adjacent versions, so this is the shape the work
actually has: a new release is a new module rather than an edit to every packet.

A packet that only one boundary changed is written directly by that module. Where several
boundaries change the same packet, the newest one lists the fields once and the modules below it
apply their own delta to what it built — dropping a field, appending one, folding two into one. A
field that moved or vanished mid-body costs one line in the module that changed it, rather than a
second copy of the field list.

The play `login` is the example: `to_26_1` lists its fields as a `Body`, and `to_1_21` takes that
body and removes the sea level 1.21.2 added.

### Bodies coming the other way

Serverbound hops run in the opposite order — the client's own version first, each hop handing the
next one up a body it understands. They work on bytes rather than on the packet type, since the
packet only exists once it has reached its native form:

```rust
pub fn client_information<R: Read>(reader: &mut R, version: ProtocolVersion) -> Upgraded
```

A hop can also say the body is a different packet now, or that it should not be acted on at all:

```rust
pub enum Upgrade { Body(Vec<u8>), Into(Vec<u8>), Dropped }
```

`Into` goes to the packet named by `#[upgrade_into(..)]` next to the translator — 26.1 split the
attack out of the interaction, so an older client's attack arrives as an interaction and has to be
dispatched as an attack. `Dropped` is for a gesture an older client reports twice.

## Ids

A hop writes its own packet id, because a downgrade is sometimes onto a different packet rather
than an older shape of the same one: what 26.2 sends as an `entity_position_sync` reaches 1.21 as a
`teleport_entity`. Where no hop fires, the id comes from the generated tables.

A packet Mojang merely renamed needs no hop; the rename is recorded next to the tables so a lookup
that misses the current name retries the older ones.

## Registry ids

Items, entity types, sounds and particles travel as bare indices into registries that grow between
releases, so those ids shift too. `NetworkItemId`, `NetworkEntityType`, `NetworkSound` and
`NetworkParticle` translate as they are encoded, from tables built by
`scripts/build_registry_remap.py`.

An id the target version has nothing for is handled differently per registry: an item takes a
stand-in from the same family, since a container slot has to hold something, while the rest refuse
to encode and the send paths drop that packet for that one recipient. A wrong entity or sound is
worse than none.

## Block states

Block state ids are a dense index over property combinations, so one added property value shifts
every id after it. Between 1.21.4 and 26.2 that moves 25765 ids onto a different block. Anything
carrying a block state uses `NetworkBlockState`, which translates as it is encoded — a block update
is broadcast to players who need not share a version, so it cannot be translated when the packet is
built.

See `src/lib/world/src/chunk/remap.rs` and `scripts/build_block_state_remap.py`.

## Testing it

`src/bin/tests/` starts a real server and checks what it says to each version. For conformance
against a real Minecraft implementation, put ViaProxy in front of the server and connect the stress
bot through it; see [Server tests](../testing/server-tests.md) and `tools/stress-bot/README.md`.
