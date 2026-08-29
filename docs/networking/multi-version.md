# Speaking to more than one client version

The server holds one world and builds every packet in its own version — currently 26.2. Clients on
older versions are served by translating on the way out, so nothing above the network layer has to
know which version a player is on.

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

Where a packet changed at more than one boundary, the lower module writes the older form directly
rather than rewriting the higher module's output. That avoids an intermediate representation at the
cost of the lower module accounting for both changes, explicitly, in one place.

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
