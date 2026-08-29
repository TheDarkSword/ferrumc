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
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROXY="${VIAPROXY_JAR:-$HOME/.minecraft/azalea-viaversion/ViaProxy-3.4.12.jar}"
PROFILE="${PROFILE:-quick}"
WORK="$(mktemp -d)"
VERSIONS=("$@")
[ ${#VERSIONS[@]} -eq 0 ] && VERSIONS=(1.21 1.21.3 1.21.4 1.21.5 1.21.6 1.21.8 1.21.10 1.21.11 26.1 26.2)

BINARY="$ROOT/target/$PROFILE/ferrumc"
[ -f "$PROXY" ]  || { echo "ViaProxy not found at $PROXY; set VIAPROXY_JAR" >&2; exit 1; }
[ -f "$BINARY" ] || { echo "no server at $BINARY; cargo build --profile $PROFILE" >&2; exit 1; }

PROXY_PIDS=()
cleanup() {
  for pid in "${PROXY_PIDS[@]:-}"; do [ -n "$pid" ] && kill "$pid" 2>/dev/null; done
  [ -n "${SERVER_PID:-}" ] && kill "$SERVER_PID" 2>/dev/null
  [ -f "$WORK/config.backup" ] && cp "$WORK/config.backup" "$CONFIG"
  rm -rf "$WORK"
}
trap cleanup EXIT

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
  java -jar "$PROXY" cli --bind-address "127.0.0.1:$port" --target-address 127.0.0.1:25565 \
    --target-version "$version" --proxy-online-mode false > "$WORK/proxy-$version.log" 2>&1 &
  PROXY_PIDS+=($!)
  port=$((port + 1))
done

port=25600
for version in "${VERSIONS[@]}"; do
  until ss -ltn 2>/dev/null | grep -q ":$port"; do sleep 1; done
  port=$((port + 1))
done

printf '%-10s %-16s %s\n' VERSION RESULT DETAIL
port=25600
for version in "${VERSIONS[@]}"; do
  before=$(grep -c 'loaded at' "$WORK/server.log")
  # A bot that is going to join does so in a couple of seconds; the rest is only waiting.
  (cd "$ROOT/tools/stress-bot" && timeout 12 cargo run -q -- \
    --server "127.0.0.1:$port" --bots 1 --stats-interval-secs 30 > "$WORK/bot-$version.log" 2>&1)
  after=$(grep -c 'loaded at' "$WORK/server.log")
  errors=$(grep -c 'ERROR IN' "$WORK/proxy-$version.log")

  if [ "$after" -gt "$before" ] && [ "$errors" -eq 0 ]; then
    printf '%-10s %-16s %s\n' "$version" "joined" "clean"
  elif [ "$after" -gt "$before" ]; then
    printf '%-10s %-16s %s\n' "$version" "joined" "$errors translation errors"
  else
    detail=$(grep -oE 'ERROR IN [A-Za-z0-9_]+ IN REMAP OF [A-Z_]+' "$WORK/proxy-$version.log" | sort -u | head -1)
    printf '%-10s %-16s %s\n' "$version" "did not join" "${detail:-no proxy error logged}"
  fi
  port=$((port + 1))
done
