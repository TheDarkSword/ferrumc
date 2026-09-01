# 🛠️ Vanilla 1:1 Parity Feature Implementation Checklist for FerrumC

> **This list has drifted from the code.** Known-stale entries: fluids, particles and several
> commands are marked missing but exist; the protocol section describes 1.21.8 while the server
> now speaks 26.2 (see [Target version](versioning/target-version.md)). Verify against the source
> before relying on any line here.

> **Legend:**
> - [x] = Done
> - [/] = Partial
> - [ ] = Not Done


## I. Core Infrastructure & Lifecycle

### **Bootstrap**
- [x] **Command Line Argument Parsing** — Uses `clap` crate in [`src/bin/src/cli.rs`](../src/bin/src/cli.rs). Supports `--log` (trace/debug/info/warn/error), subcommands: `setup`, `import --import-path`, `run`. **Missing:** `--nogui`, `--port`, `--world` flags (port/world configured via TOML instead).
- [ ] **EULA Acceptance/Enforcement** — Not implemented.
- [x] **Logging System** — Uses `tracing` + `tracing-subscriber` in [`src/lib/utils/logging/src/lib.rs`](../src/lib/utils/logging/src/lib.rs). Rolling file appenders via `tracing-appender` (daily rotation), console colors via ANSI. **DEVIATION:** Uses Rust `tracing` ecosystem instead of Log4j.
- [x] **Native Library Loading** — **DEVIATION:** Pure Rust implementations. Compression uses `yazi`/`libdeflater` crates, encryption uses `aes`/`cfb8`/`rsa` crates. No native JNI loading needed.

### **The Main Loop (The Tick)**
- [x] **The Tick Loop** — Implemented in [`src/bin/src/game_loop.rs`](../src/bin/src/game_loop.rs). Configurable TPS via `config.toml`, period calculated as `Duration::from_secs(1) / tps`. Default 20 TPS = 50ms.
- [x] **Catch-up logic** — Implemented via `MissedTickBehavior::Burst` in [`src/lib/scheduler/src/lib.rs`](../src/lib/scheduler/src/lib.rs). Supports Burst (run missed ticks up to `max_catch_up`), Skip, and Delay behaviors. Global catch-up cap of 64 schedules per iteration.
- [/] **Tick splitting** — Has separate `TimedSchedule`s for tick, world_sync (15s), keepalive (1s) with phase offsets. **Missing:** Explicit World/Connection/Task sub-tick separation like vanilla.
- [ ] **Watchdog** — Not implemented. No crash detection for hung ticks.

### **Threading Model**
- [x] **Main Thread (Logic)** — Game loop runs on main thread, Bevy ECS schedules execute `SingleThreaded`.
- [x] **Netty IO Threads (Networking)** — **DEVIATION:** Uses Tokio async TCP instead of Netty NIO. TCP acceptor spawns on dedicated thread with its own Tokio runtime in [`src/bin/src/game_loop.rs:tcp_conn_acceptor()`](../src/bin/src/game_loop.rs). Connection handlers in [`src/lib/net/src/connection.rs`](../src/lib/net/src/connection.rs).
- [x] **Chunk Loading/Gen Threads (Async)** — Chunk loading uses thread pool batches in [`src/bin/src/systems/chunk_sending.rs`](../src/bin/src/systems/chunk_sending.rs). `ferrumc_state.thread_pool.batch()` for parallel chunk generation.
- [ ] **Light Calculation Threads** — Not implemented. Light data imported from vanilla chunks but not recalculated.

### **Scheduler**
- [x] **Sync Tasks (Tick-aligned)** — `TimedSchedule` with configurable periods in [`src/lib/scheduler/src/lib.rs`](../src/lib/scheduler/src/lib.rs). Priority-based heap scheduling.
- [x] **Async Tasks** — Tokio `spawn` used throughout codebase for async operations.
- [x] **Delayed Tasks** — Supported via `TimedSchedule` with `with_phase()` for initial delay.
- [x] **Repeating Tasks** — `TimedSchedule` runs at fixed intervals with configurable `period`.

---

## II. Networking & Protocol (The Stack)

### **Transport Layer**
- [x] **TCP Listener** — [`src/lib/net/src/server.rs`](../src/lib/net/src/server.rs) binds to configurable host:port from `config.toml`. Default port 25565.
- [/] **UDP Listener (Query Protocol)** — LAN broadcast pinger implemented in [`src/bin/src/systems/lan_pinger.rs`](../src/bin/src/systems/lan_pinger.rs) (224.0.2.60:4445). **Missing:** Full Query protocol (GameSpy4 protocol).
- [x] **Netty Pipeline Configuration** — **DEVIATION:** Custom packet pipeline. Derive macros `#[derive(NetEncode, NetDecode)]` in [`src/lib/net/crates/codec/`](../src/lib/net/crates/codec/). Packet routing via `#[packet(packet_id = "...", state = "...")]` attributes.

### **Packet Handling**
- [x] **Framing: VarInt Length Prefixing** — [`src/lib/net/crates/codec/src/net_types/var_int.rs`](../src/lib/net/crates/codec/src/net_types/var_int.rs). Full VarInt read/write with async support.
- [x] **Compression: zlib** — [`src/lib/net/src/compression.rs`](../src/lib/net/src/compression.rs) using `yazi` crate (Zlib format). Configurable threshold via `network_compression_threshold` in config.
- [x] **Encryption: AES/CFB8 + RSA** — [`src/lib/net/crates/encryption/src/lib.rs`](../src/lib/net/crates/encryption/src/lib.rs). RSA 1024-bit key generation, AES-128-CFB8 stream cipher, DER public key encoding, Minecraft hex digest for session auth.
- [x] **Data Types** — All in [`src/lib/net/crates/codec/src/net_types/`](../src/lib/net/crates/codec/src/net_types/):
  - `var_int.rs` — VarInt (and VarLong support)
  - `network_position.rs` — Position (26-bit X/Z, 12-bit Y packed into i64)
  - `bitset.rs` — BitSet for chunk data
  - `length_prefixed_vec.rs` — Length-prefixed arrays
  - NBT via `ferrumc-nbt` crate (tape-based parser with derive macros)
  - JSON Text Components in [`src/lib/text/src/lib.rs`](../src/lib/text/src/lib.rs)

### **Protocol Stages**
- [x] **Handshaking** — [`src/lib/net/src/packets/incoming/handshake.rs`](../src/lib/net/src/packets/incoming/handshake.rs). Reads protocol version, server address, port, next state (1=Status, 2=Login).
- [x] **Status (SLP)** — [`src/lib/net/src/packets/incoming/status_request.rs`](../src/lib/net/src/packets/incoming/status_request.rs), [`ping_request.rs`](../src/lib/net/src/packets/incoming/ping.rs). Full JSON response with MOTD, player count, max players, favicon (base64), version info.
- [x] **Login** — [`src/lib/net/src/packets/incoming/login_start.rs`](../src/lib/net/src/packets/incoming/login_start.rs), [`encryption_response.rs`](../src/lib/net/src/packets/incoming/encryption_response.rs). Mojang session auth via `sessionserver.mojang.com/session/minecraft/hasJoined` in [`src/lib/net/src/auth.rs`](../src/lib/net/src/auth.rs). Compression packet, Login Success with UUID/username/properties.
- [/] **Play (In-Game)** — Partial implementation. **Implemented:** Chunk sending, keep-alive, player position/rotation sync, block break/place (creative), entity spawning, chat messages, command handling, sneaking state sync, arm swing animation, pick block (creative/survival), render distance syncing. **Missing:** Most gameplay packets (combat, enchanting, villagers, etc.). See [`src/lib/net/src/packets/outgoing/`](../src/lib/net/src/packets/outgoing/) for ~45 outgoing packets.
- [ ] **RCON** — Not implemented.

---

## III. World Management (The Environment)

### **File I/O & Storage**
- [/] **Level.dat** — Seed reading from `level.dat` NBT. **Missing:** World settings, time, spawn point persistence.
- [x] **Region Files (.mca)** — Full Anvil format import in [`src/lib/world/src/importing.rs`](../src/lib/world/src/importing.rs) and [`vanilla_chunk_format.rs`](../src/lib/world/src/vanilla_chunk_format.rs). Supports Zlib, GZip compression via [`src/lib/storage/src/compressors/`](../src/lib/storage/src/compressors/). **DEVIATION:** Uses LMDB (heed) for runtime storage instead of region files. See [`src/lib/storage/src/lmdb.rs`](../src/lib/storage/src/lmdb.rs).
- [x] **Player Data** — [`src/lib/world/src/player.rs`](../src/lib/world/src/player.rs), written when a player leaves. Abilities, gamemode, position, rotation, inventory, ender chest, health, hunger, experience and effects; advancements in a table of their own, as vanilla keeps them in a file of their own. **DEVIATION:** bitcode in LMDB rather than NBT files. **Missing:** spawn point, statistics and recipe book, none of which exist yet to save.

### **Chunk System**
- [x] **Chunk Structure** — [`src/lib/world/src/chunk_format.rs`](../src/lib/world/src/chunk_format.rs). 16x384x16 columns (Y range -64 to 320 for 1.18+).
- [x] **Sections** — 16x16x16 `Section` structs with y-index, block states, biome states, block/sky light.
- [x] **Paletted Containers** — [`src/lib/world/src/chunk_format.rs`](../src/lib/world/src/chunk_format.rs). `PaletteType` enum: Single (one block type), Indirect (palette + packed data), Direct (global IDs).
- [x] **Bit Storage** — [`src/lib/utils/general_purpose/src/data_packing.rs`](../src/lib/utils/general_purpose/src/data_packing.rs). Packing block IDs into i64 arrays with variable bits-per-block.
- [x] **Heightmaps** — `MOTION_BLOCKING`, `WORLD_SURFACE` calculated and serialized as NBT in [`chunk_format.rs`](../src/lib/world/src/chunk_format.rs).
- [ ] **Chunk State** — No proto-chunk vs level chunk distinction. All chunks treated as fully loaded.

### **World Generation (The Pipeline)**
- [/] **Noise Generators** — OpenSimplex noise in [`src/lib/world_gen/src/lib.rs`](../src/lib/world_gen/src/lib.rs). Multi-layer noise with configurable scales. **Missing:** Full Perlin, octave noise.
- [/] **Biome Source** — Basic biome generator trait in [`src/lib/world_gen/src/biomes/`](../src/lib/world_gen/src/biomes/). Plains biome implemented. **Missing:** Multi-noise biome selection (Temperature, Humidity, etc.).
- [/] **Surface Builders** — Plains biome generates grass/dirt surface. **Missing:** Other biome surfaces.
- [ ] **Carvers** — Not implemented. No caves/ravines.
- [ ] **Features** — Not implemented. No trees/ores/lakes.
- [ ] **Structures** — Not implemented. No jigsaw system.

### **Lighting Engine**
- [/] **Sky Light** — Imported from vanilla chunks, stored per-section. **Missing:** Runtime propagation.
- [/] **Block Light** — Imported from vanilla chunks, stored per-section. **Missing:** Runtime propagation.
- [ ] **Light Updates** — Not implemented. No light recalculation system.

### **Block Logic**
- [x] **Block Registry** — [`src/lib/registry/src/lib.rs`](../src/lib/registry/src/lib.rs). Compile-time `phf::Map` for block name ↔ protocol ID lookups. Generated from vanilla data at build time.
- [x] **Block States** — [`src/lib/world/src/block_state_id.rs`](../src/lib/world/src/block_state_id.rs). State IDs map to property combinations. **Note:** Properties not individually accessible at runtime (stored as flat IDs).
- [/] **Voxel Shapes** — Basic collision bounds in [`src/lib/core/src/collisions/`](../src/lib/core/src/collisions/). **Missing:** Per-block collision shapes.
- [/] **Material Properties** — Block hardness lookup via `lookup_block_hardness()` in registry. **Missing:** Blast resistance, flammability, etc.
- [ ] **Tick Logic** — Not implemented. No random/scheduled block ticks.

---

## IV. Entity System (The Actors)

### **Base Entity Architecture**
- [x] **Entity IDs & UUIDs** — Bevy ECS entities with numeric IDs. UUIDs stored as `PlayerIdentity` component in [`src/lib/core/src/identity/`](../src/lib/core/src/identity/).
- [x] **Position & Rotation** — [`src/lib/core/src/transform/position.rs`](../src/lib/core/src/transform/position.rs), [`rotation.rs`](../src/lib/core/src/transform/rotation.rs). `Position` wraps `DVec3`, `Rotation` has yaw/pitch with normalization.
- [/] **Velocity** — Not as dedicated component. Movement handled via position deltas.
- [x] **Hitboxes** — `BoundingBox` and `PhysicalProperties` in [`src/lib/entities/src/components/physical.rs`](../src/lib/entities/src/components/physical.rs). AABB from vanilla entity data.
- [x] **DataTracker (Metadata)** — `SyncedData` in [`src/lib/entities/src/synced_data/`](../src/lib/entities/src/synced_data/), the wire form in [`entity_metadata.rs`](../src/lib/net/src/packets/outgoing/entity_metadata.rs). Layouts and serializer numbers extracted per version; only what changed is sent.

### **Physics & Movement**
- [x] **Gravity simulation** — [`src/lib/physics/`](../src/lib/physics/), driven by [`src/bin/src/systems/physics/`](../src/bin/src/systems/physics/). Per type, extracted from the game; sixty-five of the hundred and fifty-eight types differ from the common 0.08.
- [x] **Drag/Air Resistance** — same place. Applied in vanilla's own order, which differs between mobs and dropped things and is what decides the terminal speed.
- [/] **Fluid handling** — water and lava change how an entity is pulled and slowed. **Missing:** fluid height, so being in one is decided by the block at the entity's feet; and bubble columns.
- [ ] **Piston pushing logic** — Not implemented.
- [x] **Collision resolution** — [`src/bin/src/systems/physics/collisions.rs`](../src/bin/src/systems/physics/collisions.rs). Swept per axis against the real voxel shapes, with the step-up that walks an entity onto a slab.

### **Player Entity**
- [x] **Gamemodes** — [`src/lib/components/src/player/gamemode.rs`](../src/lib/components/src/player/gamemode.rs). All 4 modes: Survival, Creative, Adventure, Spectator. Configurable default in config.
- [x] **Abilities** — [`src/lib/components/src/player/abilities.rs`](../src/lib/components/src/player/abilities.rs). Flying, invulnerable, may_fly, creative_mode, may_build, flying_speed, walking_speed. Mode-specific ability presets.
- [/] **Food/Exhaustion/Saturation** — `Hunger` component in [`src/lib/components/src/player/hunger.rs`](../src/lib/components/src/player/hunger.rs) with level/saturation/exhaustion. **Missing:** Exhaustion tick logic.
- [/] **XP/Leveling** — `Experience` component in [`src/lib/components/src/player/experience.rs`](../src/lib/components/src/player/experience.rs) with progress/level/total_xp. **Missing:** Level calculation formulas.
- [ ] **Statistics tracking** — Not implemented.
- [ ] **Advancements** — Not implemented.

### **Spawning**
- [x] **Natural spawning** — [`src/lib/spawning/`](../src/lib/spawning/), driven by [`src/bin/src/systems/mobs/natural.rs`](../src/bin/src/systems/mobs/natural.rs). The per-chunk attempt loop, pack spawning and the distance rules, in vanilla's order.
- [x] **Mob caps** — both of vanilla's: one against how much of the world is loaded, one against the chunks around a place.
- [x] **Spawn placement** — where a kind may stand and what the place has to be like, both extracted from the game. Seven conditions cover fifty types; the rest belong to a mob each and are refused until it exists.
- [x] **Despawning** — by the category's despawn distance, with the roll vanilla gives a mob between the two distances.
- [x] **Chunk-generation spawning** — a chunk is given its animals the first time a player sees it, and carries a mark saying so. Not at generation, which happens off the tick thread where nothing can be spawned.
- [ ] **Spawner blocks** — Not implemented. Rules of their own, and they need block entities that tick.

### **AI (Mob Brains)**
- [ ] **Pathfinding** — Not implemented.
- [ ] **Goal Selector** — Not implemented.
- [ ] **Sensing** — Not implemented.

### **Persistence**
- [x] **Entity storage** — [`src/lib/world/src/entities.rs`](../src/lib/world/src/entities.rs) and [`src/bin/src/systems/mobs/persistence.rs`](../src/bin/src/systems/mobs/persistence.rs). Entities are written with the chunk they stand in, in a table of their own; a chunk coming into view brings them back with the names they had. **Missing:** what a mob is beyond where it is, since no mob has any state of its own yet.
- [ ] **Vanilla entity import** — Not implemented. An imported world's chunks come across and its mobs do not.

### **Entity Categories**
- [/] **Living** — `Health` component in [`src/lib/components/src/health.rs`](../src/lib/components/src/health.rs). **Missing:** Potions, armor, hand items.
- [/] **Mobs** — every type exists and can be spawned, naturally or by command, with the physics and synced data the game gives it. **Missing:** behaviour, and the thirty-three types whose spawn condition is their own.
- [ ] **Projectiles** — Not implemented.
- [ ] **Vehicles** — Not implemented.
- [ ] **Items** — Not implemented. No item entities.
- [ ] **Displays** — Not implemented.

---

## V. Inventory & GUI System

### **Item Stack**
- [x] **Item Type, Count** — [`src/lib/inventories/src/item.rs`](../src/lib/inventories/src/item.rs). `ItemID` with `VarInt` protocol ID, name lookups via registry.
- [/] **Components/NBT** — Slot data supports NBT components. **Missing:** Full component system (enchantments, lore, etc.).

### **Container Logic**
- [x] **Slots** — [`src/lib/inventories/src/slot.rs`](../src/lib/inventories/src/slot.rs), [`defined_slots.rs`](../src/lib/inventories/src/defined_slots.rs). Slot indexing with item optional.
- [/] **Window Types** — Player inventory only. **Missing:** Chest, furnace, anvil, etc.
- [/] **Synchronization** — Container content/slot packets in [`src/lib/net/src/packets/outgoing/`](../src/lib/net/src/packets/outgoing/). Click handling in [`src/bin/src/packet_handlers/play_packets/`](../src/bin/src/packet_handlers/play_packets/).

### **Recipe System**
- [ ] **Recipe Registry** — Not implemented.
- [ ] **Recipe Types** — Not implemented.
- [ ] **Recipe Book** — Not implemented.

---

## VI. Gameplay Mechanics (The Logic)

### **Interaction Processing**
- [x] **Player Digging** — [`src/bin/src/packet_handlers/play_packets/player_action.rs`](../src/bin/src/packet_handlers/play_packets/player_action.rs). Creative insta-break works. Survival digging events emitted. **Missing:** Break speed calculation, tool efficiency.
- [x] **Block Placing** — [`src/bin/src/packet_handlers/play_packets/place_block.rs`](../src/bin/src/packet_handlers/play_packets/place_block.rs). Item-to-block mapping, face-based offset calculation, collision checking with players.
- [ ] **Item Usage** — Not implemented. No eating/bow/shield.

### **Redstone Engine**
- [ ] **Signal strength propagation** — Not implemented.
- [ ] **Quasi-connectivity** — Not implemented.
- [ ] **Update Order** — Not implemented.
- [ ] **Component logic** — Not implemented.

### **Fluid Dynamics**
- [ ] **Water/Lava spreading** — Not implemented.
- [ ] **Source block formation** — Not implemented.
- [ ] **Fluid mixing** — Not implemented.

### **Combat System**
- [ ] **Attack Cooldown** — Not implemented.
- [ ] **Damage Sources** — Not implemented.
- [ ] **Armor formulas** — Not implemented.
- [/] **Knockback** — the arithmetic in [`src/lib/physics/`](../src/lib/physics/). **Missing:** a caller, since nothing deals damage yet.
- [ ] **Critical Hits** — Not implemented.

### **Magic & Effects**
- [/] **Status Effects** — `ActiveEffects` component in [`src/lib/components/src/active_effects.rs`](../src/lib/components/src/active_effects.rs). **Missing:** Effect tick logic.
- [ ] **Enchantments** — Not implemented.
- [ ] **Potions** — Not implemented.

---

## VII. Command & Chat System

### **Chat**
- [x] **JSON Chat Components** — [`src/lib/text/src/lib.rs`](../src/lib/text/src/lib.rs). Full `TextComponent` with text/translate/keybind content, colors, formatting, click/hover events, NBT serialization.
- [/] **Chat Messages** — [`src/bin/src/packet_handlers/play_packets/chat_message.rs`](../src/bin/src/packet_handlers/play_packets/chat_message.rs). Basic `<player> message` broadcast via message queue.
- [ ] **Chat Signing (1.19+)** — Not implemented.

### **Brigadier (Command Parser)**
- [x] **Command Tree** — [`src/lib/commands/src/graph/`](../src/lib/commands/src/graph/). Command nodes with argument types, suggestions.
- [x] **Argument Types** — [`src/lib/commands/src/arg/`](../src/lib/commands/src/arg/). String, integer, and custom argument parsers.
- [/] **Target Selectors** — Not implemented as full selector system.
- [ ] **Permission Levels** — Not implemented.
- [/] **Command Logic** — [`src/lib/default_commands/src/`](../src/lib/default_commands/src/). Implemented: `/echo`, `/fly`, `/gamemode`, `/spawn`, `/nested` (test). **Missing:** Most vanilla commands.

---

### **Datapack Loader**
- [ ] **Zip/Folder parsing** — Not implemented.
- [ ] **pack.mcmeta reading** — Not implemented.
- [ ] **Functions (.mcfunction)** — Not implemented.
- [ ] **Loot Tables** — Not implemented.
- [ ] **Predicates** — Not implemented.
- [ ] **Tags** — Not implemented.

---

## IX. Scoreboard & Teams
- [ ] **Objectives** — Not implemented.
- [ ] **Scores** — Not implemented.
- [ ] **Teams** — Not implemented.
- [ ] **Display Slots** — Not implemented.

---

## X. Block Entities (Tile Entities)
- [ ] **Tickable Block Entities** — Not implemented.
- [ ] **Passive Block Entities** — Not implemented.
- [ ] **Complex Logic** — Not implemented.

---

## XI. Miscellaneous Subsystems
- [ ] **Raids** — Not implemented.
- [ ] **Patrols** — Not implemented.
- [ ] **Villager Gossiping** — Not implemented.
- [ ] **Explosions** — Not implemented.
- [ ] **Maps** — Not implemented.
- [ ] **Statistics** — Not implemented.
- [ ] **Boss Bars** — Not implemented.
- [ ] **Particles** — Not implemented.
- [ ] **Sounds** — Not implemented.

---

## XII. Security & Anti-Cheat (Vanilla)
- [ ] **Packet Limiter** — Not implemented.
- [ ] **Movement Checks** — Not implemented.
- [ ] **Chat Validation** — Not implemented.
- [ ] **Spam Filtering** — Not implemented.

---

# Significant Deviations from Vanilla Java Server
(This is just for reference for like funsies or something)

| Area | Vanilla (Java) | FerrumC (Rust) | Notes |
|------|----------------|----------------|-------|
| **Runtime** | JVM with GC | Native binary | No garbage collection, manual memory via Rust ownership. Faster startup, lower memory overhead. |
| **Async I/O** | Netty (Java NIO) | Tokio | Rust async/await with `mio` underneath. Connection handling in [`src/lib/net/src/connection.rs`](../src/lib/net/src/connection.rs). |
| **ECS** | Object-oriented entities | Bevy ECS | Archetype-based component storage. Entities are just IDs, components attached separately. See [`src/lib/entities/`](../src/lib/entities/). |
| **NBT** | Mojang's NBT lib | `ferrumc-nbt` | Custom tape-based parser with derive macros (`#[derive(NBTSerialize, NBTDeserialize)]`). Zero-copy parsing where possible. |
| **Config** | `server.properties` | TOML via `figment` | [`src/lib/config/src/server_config.rs`](../src/lib/config/src/server_config.rs). Hierarchical config merging (defaults + file override). |
| **Storage Backend** | Anvil (.mca) files | LMDB (heed) | [`src/lib/storage/src/lmdb.rs`](../src/lib/storage/src/lmdb.rs). Memory-mapped B+tree database. Import from Anvil supported. Configurable map size in GB. |
| **Packet Codec** | Hand-written | Derive macros | `#[derive(NetEncode, NetDecode)]` with `#[packet(...)]` attributes in [`src/lib/net/crates/codec/`](../src/lib/net/crates/codec/). |
| **World Import** | N/A | Dedicated CLI | `ferrumc import --import-path <path>` for batch chunk import from vanilla worlds. |
| **Logging** | Log4j | tracing | Zero-cost when disabled, structured logging, daily file rotation in `logs/`. |
| **Compression** | JDK zlib | yazi (Rust) | Pure Rust Zlib implementation. [`src/lib/net/src/compression.rs`](../src/lib/net/src/compression.rs). |
| **Encryption** | JCE/BouncyCastle | rsa/aes crates | Pure Rust crypto. RSA 1024-bit, AES-128-CFB8. [`src/lib/net/crates/encryption/`](../src/lib/net/crates/encryption/). |
| **Scheduling** | Single tick loop | Multi-schedule system | `ferrumc-scheduler` with independent schedules (tick, world_sync, keepalive) and missed-tick behaviors. |
| **Registry** | Runtime JSON parsing | Compile-time PHF maps | Block/item registries generated at build time via `build.rs`. Instant O(1) lookups. [`src/lib/registry/`](../src/lib/registry/). |
| **Chunk Cache** | Soft references | Moka cache | Weighted LRU cache with configurable TTL and capacity. [`src/lib/world/src/lib.rs`](../src/lib/world/src/lib.rs). |
| **Thread Pool** | Fork-join pool | Custom batch executor | Thread pool with `batch()` API for parallel chunk operations. |
| **Entity Data** | Static Java classes | Generated from JSON | Entity types, dimensions, and properties extracted from vanilla and accessed via `ferrumc-data` crate. |