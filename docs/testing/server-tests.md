# Server tests

Integration tests under `src/bin/tests/` start a real server and talk to it over a socket. They
exist for behaviour that cannot be reached by calling a function: which protocol version the server
reports, what a login exchange looks like, whether a packet has the right shape for the client
reading it.

Before this, that was checked by hand — start a server, start a proxy, run a bot, read the logs —
which meant it was checked once and then drifted. These run with `cargo test`.

## Writing one

```rust
mod common;

use common::TestServer;
use ferrumc_net_codec::version::ProtocolVersion;

#[test]
fn the_server_says_hello_in_the_client_s_own_version() {
    let server = TestServer::shared();
    let mut client = server.connect().expect("connects");

    let status = client.status(ProtocolVersion::V1_21_4.number()).expect("status");

    assert_eq!(status["version"]["protocol"], 769);
}
```

## How the harness works

`TestServer` copies the built server binary into a temporary directory, writes a configuration
beside it, and starts it there. The copy is necessary because the server resolves its configuration
and world relative to its own executable, so that directory *is* the instance.

- **`TestServer::shared()`** — one instance per test binary, started on first use. Use it for tests
  that only read.
- **`TestServer::start()`** — a fresh instance with its own world, for tests that change it.

Starting a server takes a couple of seconds, so prefer `shared()` and group related assertions.

The instance runs with encryption, online mode and compression all off, so a test can log in
without a Mojang account and read frames without inflating them. A test that needs any of those has
to start its own server and teach `TestClient` about it.

Ports are picked by binding one and releasing it, which two servers starting at once can win
simultaneously. The loser fails to bind and exits, which the harness notices by checking its own
child is still alive, and retries on another port.

`PR_SET_PDEATHSIG` makes a server die with the test process, so an interrupted run does not leave
one holding a port.

## Why the client is hand-written

`TestClient` writes its own varints and frames rather than reusing `ferrumc-net-codec`. A client
built out of the server's own encoder agrees with it by construction and could never catch it being
wrong. Everything above the frame is read as plain bytes and checked against what the protocol says
should be there.

It is deliberately small and grows one test at a time. It currently covers the handshake, server
list ping, and login up to `login_finished`.

## What it does not replace

The harness speaks the protocol the way this server implements it, so it cannot prove a real
Minecraft client agrees. For that, put ViaProxy in front of the server and connect the stress bot
through it — see `tools/stress-bot/README.md`. Use the harness for behaviour the server owns, and
the proxy for conformance against a real implementation.
