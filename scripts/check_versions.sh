#!/usr/bin/env bash
# Connects a bot to the server as each supported client version, through ViaProxy, and reports
# whether it joined and whether the proxy could translate what the server sent.
#
# The in-process tests in src/bin/tests/ check what the server says; this checks that a real
# Minecraft implementation agrees. See docs/testing/server-tests.md.
#
# Usage: scripts/check_versions.sh [version ...]     (default: every supported version)
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROXY="${VIAPROXY_JAR:-$HOME/.minecraft/azalea-viaversion/ViaProxy-3.4.12.jar}"
WORK="$(mktemp -d)"
VERSIONS=("$@")
[ ${#VERSIONS[@]} -eq 0 ] && VERSIONS=(1.21 1.21.3 1.21.4 1.21.5 1.21.6 1.21.8 1.21.10 1.21.11 26.1 26.2)

[ -f "$PROXY" ] || { echo "ViaProxy not found at $PROXY; set VIAPROXY_JAR" >&2; exit 1; }

cleanup() {
  [ -n "${PROXY_PID:-}" ] && kill "$PROXY_PID" 2>/dev/null
  [ -n "${SERVER_PID:-}" ] && kill "$SERVER_PID" 2>/dev/null
  rm -rf "$WORK"
}
trap cleanup EXIT

"$ROOT/target/release/ferrumc" --log=info > "$WORK/server.log" 2>&1 &
SERVER_PID=$!
until ss -ltn 2>/dev/null | grep -q ':25565'; do sleep 1; done

printf '%-10s %-22s %s\n' VERSION RESULT DETAIL
for version in "${VERSIONS[@]}"; do
  java -jar "$PROXY" cli --bind-address 127.0.0.1:25568 --target-address 127.0.0.1:25565 \
    --target-version "$version" --proxy-online-mode false > "$WORK/proxy-$version.log" 2>&1 &
  PROXY_PID=$!
  until ss -ltn 2>/dev/null | grep -q ':25568'; do sleep 2; done

  before=$(grep -c 'loaded at' "$WORK/server.log")
  (cd "$ROOT/tools/stress-bot" && timeout 35 cargo run -q -- \
    --server 127.0.0.1:25568 --bots 1 --stats-interval-secs 30 > "$WORK/bot-$version.log" 2>&1)
  after=$(grep -c 'loaded at' "$WORK/server.log")
  errors=$(grep -c 'ERROR IN' "$WORK/proxy-$version.log")

  kill "$PROXY_PID" 2>/dev/null; PROXY_PID=
  until ! ss -ltn 2>/dev/null | grep -q ':25568'; do sleep 1; done

  if [ "$after" -gt "$before" ] && [ "$errors" -eq 0 ]; then
    printf '%-10s %-22s %s\n' "$version" "joined" "clean"
  elif [ "$after" -gt "$before" ]; then
    printf '%-10s %-22s %s\n' "$version" "joined" "$errors translation errors"
  else
    detail=$(grep -oE 'ERROR IN [A-Za-z0-9_]+ IN REMAP OF [A-Z_]+' "$WORK/proxy-$version.log" | sort -u | head -1)
    printf '%-10s %-22s %s\n' "$version" "did not join" "${detail:-no proxy error logged}"
  fi
done
