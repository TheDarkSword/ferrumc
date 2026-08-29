# Testing

| Document | What it covers |
|---|---|
| [Server tests](server-tests.md) | Tests that need a running server: how they work and how to add one |

Most behaviour is covered by ordinary unit tests inside the crate that owns it, run with the rest of
the suite:

```bash
cargo nextest run --all-targets --all-features -E "not kind(bench)"
```

Anything that only shows up on a socket — version negotiation, the login exchange, packet layout —
needs a server actually running, and lives in `src/bin/tests/`. See [Server tests](server-tests.md).

`src/tests/` is an old crate that is not a workspace member and no longer compiles against the
current API. Nothing runs it; do not add to it.
