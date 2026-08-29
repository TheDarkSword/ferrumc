# FerrumC stress-bot

A dev-only load-testing tool that spawns a swarm of [azalea](https://github.com/azalea-rs/azalea)
bots to connect to a running FerrumC server. It reproduces the chunk-loading, movement, and
broadcast load of many simultaneous players so server tick performance can be measured under real
concurrency.

## Why it is a separate workspace

This crate is **not** part of the main FerrumC workspace (it is listed under `exclude` in the root
`Cargo.toml` and carries its own `[workspace]`). azalea pulls in a large dependency tree, and the
main build, CI, and release must not pay that cost. Build and run it explicitly from this
directory.

## Version compatibility

Pinned to an exact azalea git revision that targets **Minecraft 26.2 (protocol 776)** — the version
FerrumC speaks. The published releases stop at `0.16.0+mc26.1`, so 26.2 is only reachable from the
repository's main branch. When FerrumC's protocol version changes, move the pin to a revision whose
README names the matching Minecraft version, or to a published release once one covers it.

Because it speaks the server's own protocol version, this bot doubles as the connection smoke test:
if a single bot reaches the play state and receives chunks, the handshake, login, configuration and
registry payloads are all well formed.

azalea's bitset relies on `generic_const_exprs`, which the new trait solver cannot yet resolve, so
`.cargo/config.toml` here repeats the `-Znext-solver=coherence` flag azalea sets for itself — a git
dependency's own cargo config does not reach the workspace consuming it. `rust-toolchain.toml` pins
a dated nightly for the same reason.

## Usage

Start a FerrumC server in offline mode, then:

```bash
cd tools/stress-bot

# 100 wandering bots, joining 150ms apart
cargo run --release -- --server 127.0.0.1:25565 --bots 100 --join-delay-ms 150

# 200 bots that only connect and idle (pure connection/keepalive/chunk-tracking load)
cargo run --release -- --bots 200 --idle
```

### Options

| Flag | Default | Meaning |
|------|---------|---------|
| `--server <host:port>` | `127.0.0.1:25565` | Server to connect to |
| `--bots <N>` | `50` | Number of bots to spawn |
| `--join-delay-ms <ms>` | `200` | Delay between successive bot joins |
| `--name-prefix <s>` | `stress` | Bots are named `<prefix>_<n>` |
| `--idle` | off | Connect and idle without moving |
| `--stats-interval-secs <s>` | `5` | Seconds between metrics reports |
| `--reconnect-delay-secs <s>` | `5` | Delay before reconnecting a dropped bot |

Set `RUST_LOG` (e.g. `RUST_LOG=info`) for azalea's own logging; by default the tool only prints its
periodic metrics line:

```
[stress] connected=100 peak=100 joins=104 disconnects=4
```

## Dependency pinning (important)

azalea 0.14 depends on a set of **pre-release** RustCrypto crates (`rsa 0.10.0-rc.9` and the
`der` / `pkcs1` / `pkcs8` / `spki` / `crypto-bigint` / `crypto-primes` / `rand_core` pre-releases it
was built against). Several of those crate lines have since published newer pre-releases and stable
versions with changed APIs, so a *fresh* resolution no longer compiles. The committed `Cargo.lock`
pins the whole stack back to the azalea-0.14-era versions and is therefore **load-bearing — do not
delete it or run `cargo update`** on this crate.

If the lock is ever lost or regenerated, restore the pins with:

```bash
cargo update -p rand_core    --precise 0.10.0-rc-2
cargo update -p crypto-bigint --precise 0.7.0-rc.8
cargo update -p crypto-primes --precise 0.7.0-pre.2
cargo update -p der          --precise 0.8.0-rc.7
cargo update -p pkcs1        --precise 0.8.0-rc.3
cargo update -p pkcs8        --precise 0.11.0-rc.6
cargo update -p spki         --precise 0.8.0-rc.4
```

(These also disappear when the azalea pin is bumped to a future release whose dependencies have
gone stable; at that point the pins can simply be dropped.)

## Measuring server impact

Run the server with its TPS/tick instrumentation (or the dashboard) and watch tick duration as the
bot count climbs. Comparing tick time at a fixed bot count across server revisions is the intended
way to validate performance work (storage locking, fluid ticks, world generation, …).

## Testing an older protocol version

The bot always speaks azalea's own version. To exercise the server's older-version paths, put
[ViaProxy] in front of it: the bot connects to the proxy, and the proxy speaks the version named by
`--target-version` to the server.

```bash
java -jar ViaProxy-<version>.jar cli \
  --bind-address 127.0.0.1:25568 \
  --target-address 127.0.0.1:25565 \
  --target-version 1.21.4 \
  --proxy-online-mode false

cargo run -- --server 127.0.0.1:25568 --bots 1
```

`--list-versions` prints the names it accepts.

ViaProxy is a separate process and is never linked into FerrumC, so its GPLv3 licence does not
reach this project — the same relationship as with any other build or test tool.

The `azalea-viaversion` plugin wraps the same proxy and would be tidier, but it does not currently
work: its `handle_change_address` startup system reads a `Swarm` resource that no longer exists in
the azalea revision this tool is pinned to. Revisit once it catches up.

[ViaProxy]: https://github.com/ViaVersion/ViaProxy
