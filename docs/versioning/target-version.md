# Target version

The server speaks **Minecraft 26.2**, network protocol **776**, world data version **4903**.

## Where the version appears

| Constant | Location | Value |
|---|---|---|
| Network protocol | `src/lib/net/src/conn_init/mod.rs` (`PROTOCOL_VERSION`) | 776 |
| Version name in the status response | `src/lib/net/src/conn_init/status.rs` | `26.2` |
| World data version | `src/lib/world/src/importing.rs` (`SUPPORTED_DATA_VERSION`) | 4903 |
| Packet ids | `assets/data/packets.json` | extracted from the 26.2 server jar |
| Synced registries | `assets/data/registry_packets.json` | built from the 26.2 datapack |

Packet ids are resolved at compile time by the `#[packet(...)]` macro reading
`assets/data/packets.json`, so replacing that file renumbers every packet at once. Field layouts
are not covered by it and have to be checked by hand against the release's own packet classes.

## Moving to a new version

1. Extract the new release with `scripts/extract_assets.py <version>`; it prints the protocol and
   world data version it found.
2. Copy `assets/extracted/<version>/reports/packets.json` over `assets/data/packets.json`.
3. Rebuild the synced registries with `scripts/build_registry_packets.py assets/extracted/<version>`.
   The list of synchronized registries lives in that script and changes between releases.
4. Update the three constants in the table above.
5. Build. A packet that no longer exists fails at compile time, because the macro cannot resolve
   its name.
6. Connect a client of the new version. Renumbering is automatic; **field layout changes are not**,
   and only a real client finds them. `tools/stress-bot` speaks the server's protocol version and
   is the cheapest way to do this.

Step 6 is not optional. The 772 to 776 move compiled cleanly and passed the whole test suite while
five packets were still wrong on the wire.
