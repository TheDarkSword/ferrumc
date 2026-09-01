#!/usr/bin/env bash
# Connects a bot to the server as each supported client version, through ViaProxy, and reports
# whether it joined and whether the proxy could translate what the server sent.
#
# The in-process tests in src/bin/tests/ check what the server says; this checks that a real
# Minecraft implementation agrees. See docs/testing/server-tests.md.
#
# Usage: scripts/check_versions.sh [version ...]     (default: every supported version)
#   PROFILE=release   which cargo profile's binary to run (default: quick)
#   WORLD_SEED=n      pin terrain, so two runs see the same chunks
#   FERRUMC_MATRIX_LOGS=dir  keep the server, proxy and bot logs there instead of a temp dir
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROXY="${VIAPROXY_JAR:-$HOME/.minecraft/azalea-viaversion/ViaProxy-3.4.12.jar}"
PROFILE="${PROFILE:-quick}"
WORK="${FERRUMC_MATRIX_LOGS:-$(mktemp -d)}"
mkdir -p "$WORK"
VERSIONS=("$@")
[ ${#VERSIONS[@]} -eq 0 ] && VERSIONS=(1.21 1.21.3 1.21.4 1.21.5 1.21.6 1.21.8 1.21.10 1.21.11 26.1 26.2)

BINARY="$ROOT/target/$PROFILE/ferrumc"
[ -f "$PROXY" ]  || { echo "ViaProxy not found at $PROXY; set VIAPROXY_JAR" >&2; exit 1; }

# Built here rather than assumed: this profile is not the one anything else builds, so a binary
# left from an earlier day reports a clean run for code that was never in it.
echo "building the $PROFILE profile" >&2
cargo build --profile "$PROFILE" --quiet || { echo "build failed" >&2; exit 1; }
[ -f "$BINARY" ] || { echo "no server at $BINARY after building" >&2; exit 1; }

PROXY_PIDS=()
cleanup() {
  for pid in "${PROXY_PIDS[@]:-}"; do [ -n "$pid" ] && kill "$pid" 2>/dev/null; done
  [ -n "${SERVER_PID:-}" ] && kill "$SERVER_PID" 2>/dev/null
  [ -f "$WORK/config.backup" ] && cp "$WORK/config.backup" "$CONFIG"
  # Logs are kept when a place for them was named, so a run that reported an error can be read.
  [ -n "${FERRUMC_MATRIX_LOGS:-}" ] || rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

# A pinned seed and a world of its own, so two runs see the same chunks and one version's result
# can be compared against another's. A world remembers its seed, so the old one has to go.
mkdir -p "$ROOT/target/$PROFILE/configs"
CONFIG="$ROOT/target/$PROFILE/configs/config.toml"
[ -f "$CONFIG" ] && cp "$CONFIG" "$WORK/config.backup"
cat > "$CONFIG" <<CFG
host = "127.0.0.1"
port = 25565
online_mode = false
world_seed = ${WORLD_SEED:-1234567890}
CFG
rm -rf "$ROOT/target/$PROFILE/world"

"$BINARY" --log=info > "$WORK/server.log" 2>&1 &
SERVER_PID=$!
until ss -ltn 2>/dev/null | grep -q ':25565'; do sleep 1; done

# ViaProxy spends twenty-odd seconds loading its mapping tables, so every version's proxy is
# started at once on a port of its own rather than one after another.
port=25600
for version in "${VERSIONS[@]}"; do
  # Each proxy rewrites its own config files on startup, through a temp file and a rename, so two
  # of them sharing a working directory race and the loser dies on a file that is no longer there.
  mkdir -p "$WORK/run-$version"
  # `exec` so the subshell is replaced by the proxy rather than becoming its parent: without it the
  # recorded pid is the subshell's, killing it leaves the proxy orphaned, and ten of those are a
  # good few gigabytes of jvm left behind on every run.
  (cd "$WORK/run-$version" && exec java -jar "$PROXY" cli --bind-address "127.0.0.1:$port" \
    --target-address 127.0.0.1:25565 --target-version "$version" --proxy-online-mode false \
    > "$WORK/proxy-$version.log" 2>&1) &
  PROXY_PIDS+=($!)
  port=$((port + 1))
done

port=25600
DEAD=()
for version in "${VERSIONS[@]}"; do
  # A proxy that failed to start would otherwise be waited on for ever.
  waited=0
  until ss -ltn 2>/dev/null | grep -q ":$port"; do
    sleep 1
    waited=$((waited + 1))
    if [ "$waited" -ge 90 ]; then
      DEAD+=("$version")
      break
    fi
  done
  port=$((port + 1))
done

# The bot is given a short run per version, so it must not spend it compiling: whichever version
# went first would fail for that reason alone.
(cd "$ROOT/tools/stress-bot" && cargo build -q)

printf '%-10s %-16s %s\n' VERSION RESULT DETAIL
port=25600
for version in "${VERSIONS[@]}"; do
  # Whether the bot reached the play state, which every version can report. The server's own
  # "loaded at" cannot be used: it comes from `player_loaded`, a packet 1.21 and 1.21.2 do not have.
  # A bot that is going to join does so in a couple of seconds; the rest is only waiting.
  if [[ " ${DEAD[*]} " == *" $version "* ]]; then
    printf '%-10s %-16s %s\n' "$version" "no proxy" "its proxy never came up"
    port=$((port + 1))
    continue
  fi
  run_bot() {
    (cd "$ROOT/tools/stress-bot" && timeout "$1" cargo run -q -- \
      --server "127.0.0.1:$port" --bots 1 --stats-interval-secs 3 > "$WORK/bot-$version.log" 2>&1)
    joined=$(grep -oE 'joins=[0-9]+' "$WORK/bot-$version.log" | tail -1 | cut -d= -f2)
    joined=${joined:-0}
  }

  run_bot 12
  errors=$(grep -c 'ERROR IN' "$WORK/proxy-$version.log")
  # Ten proxies are running at once, each its own JVM, and a slow machine can take longer than the
  # short run allows. A version that neither joined nor made the proxy complain is given more time
  # before it is called a failure, so a busy machine does not read as a broken protocol.
  if [ "$joined" -eq 0 ] && [ "$errors" -eq 0 ]; then
    run_bot 30
    errors=$(grep -c 'ERROR IN' "$WORK/proxy-$version.log")
  fi

  if [ "$joined" -gt 0 ] && [ "$errors" -eq 0 ]; then
    printf '%-10s %-16s %s\n' "$version" "joined" "clean"
  elif [ "$joined" -gt 0 ]; then
    printf '%-10s %-16s %s\n' "$version" "joined" "$errors translation errors"
  else
    detail=$(grep -oE 'ERROR IN [A-Za-z0-9_]+ IN REMAP OF [A-Z_]+' "$WORK/proxy-$version.log" | sort -u | head -1)
    printf '%-10s %-16s %s\n' "$version" "did not join" "${detail:-no proxy error logged}"
  fi
  port=$((port + 1))
done
