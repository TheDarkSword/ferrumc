#!/usr/bin/env python3
"""Build assets/data/block_state_remap/<version>.bin.

Block state ids are a dense index over every block's property combinations, so adding one property
value to one block shifts every id after it. Between 1.21.4 and 26.2 that moves 25765 ids onto a
different block entirely: a client of the older version reads 26.2's `diamond_ore` as
`jungle_hanging_sign`.

The table maps each of the server's own (26.2) block state ids to the id meaning the same thing in
an older version. Where the exact state does not exist there, the fallbacks are, in order:

1. the same block with its default properties, for a state that only gained a property value;
2. a block of the same family, found from the longest shared name suffix, keeping the properties
   where it has the same ones;
3. `minecraft:stone`, for anything the first two cannot place.

Usage:
    scripts/build_block_state_remap.py 1.21.4
    scripts/build_block_state_remap.py --all

Output is a flat little-endian u16 array indexed by the server's own block state id.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
EXTRACTED = REPO_ROOT / "assets" / "extracted"
DEFAULT_OUT_ROOT = REPO_ROOT / "assets" / "data" / "block_state_remap"

# The version the server's own world model uses; every table maps from it.
NATIVE = "26.2"
FALLBACK_BLOCK = "minecraft:stone"


def substitute_block(name: str, available: set[str]) -> str | None:
    """Pick a block in `available` to stand in for one that version does not have.

    A block's name ends in what it behaves like - `_stairs`, `_slab`, `_bars`, `_torch`, `_chest` -
    so the longest name suffix shared with an existing block picks something of the right shape:
    `copper_bars` becomes `iron_bars`, `cinnabar_brick_stairs` becomes `brick_stairs`. Shape and
    collision matter more to a client than texture, and getting them wrong is what drops a player
    through the floor.

    Candidates are ranked by how much of the name they share, then by brevity, which prefers the
    plain member of a family over a decorated one.
    """
    tokens = name.split(":", 1)[1].split("_")
    for start in range(len(tokens)):
        suffix = "_".join(tokens[start:])
        exact = f"minecraft:{suffix}"
        if exact in available:
            return exact
        candidates = [
            block
            for block in available
            if block.split(":", 1)[1].endswith(f"_{suffix}")
        ]
        if candidates:
            wanted = set(tokens)
            return min(
                candidates,
                key=lambda c: (-len(wanted & set(c.split(":", 1)[1].split("_"))), len(c), c),
            )
    return None


def load_states(version: str) -> tuple[dict[int, tuple], dict[tuple, int], dict[str, int]]:
    """Return id -> state key, state key -> id, and block name -> default state id."""
    report = EXTRACTED / version / "reports" / "blocks.json"
    data = json.loads(report.read_text(encoding="utf-8"))

    by_id, by_key, defaults = {}, {}, {}
    for block, info in data.items():
        for state in info["states"]:
            key = (block, tuple(sorted(state.get("properties", {}).items())))
            by_id[state["id"]] = key
            by_key[key] = state["id"]
            if state.get("default"):
                defaults[block] = state["id"]
    return by_id, by_key, defaults


def build_table(target: str) -> tuple[list[int], dict[str, int]]:
    native_by_id, _, _ = load_states(NATIVE)
    _, target_by_key, target_defaults = load_states(target)

    available = set(target_defaults)
    fallback = target_defaults[FALLBACK_BLOCK]
    table = [0] * (max(native_by_id) + 1)
    counts = {"exact": 0, "default_state": 0, "family": 0, "stone": 0}
    chosen: dict[str, str | None] = {}

    for state_id in range(len(table)):
        key = native_by_id.get(state_id)
        if key is None:
            table[state_id] = fallback
            counts["stone"] += 1
            continue

        block, properties = key
        if key in target_by_key:
            table[state_id] = target_by_key[key]
            counts["exact"] += 1
            continue
        if block in target_defaults:
            table[state_id] = target_defaults[block]
            counts["default_state"] += 1
            continue

        if block not in chosen:
            chosen[block] = substitute_block(block, available)
        stand_in = chosen[block]
        if stand_in is None:
            table[state_id] = fallback
            counts["stone"] += 1
            continue

        # Carry the properties across where the stand-in has the same ones, so a replaced stair
        # keeps its facing and half rather than snapping to the default.
        table[state_id] = target_by_key.get(
            (stand_in, properties), target_defaults[stand_in]
        )
        counts["family"] += 1

    return table, counts


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", nargs="?", help="target version, e.g. 1.21.4")
    parser.add_argument("--all", action="store_true", help="build a table for every extraction")
    parser.add_argument("--out-dir", type=Path, default=DEFAULT_OUT_ROOT)
    args = parser.parse_args()

    if args.all:
        versions = sorted(
            p.name for p in EXTRACTED.iterdir() if (p / "reports" / "blocks.json").is_file()
        )
    elif args.version:
        versions = [args.version]
    else:
        parser.error("a version is required unless --all is given")

    args.out_dir.mkdir(parents=True, exist_ok=True)
    for version in versions:
        table, counts = build_table(version)
        # Written as little-endian u16 rather than JSON: the tables are read straight out of the
        # binary with include_bytes!, and 32k numbers per version would otherwise be 32k tokens for
        # the compiler to chew through, ten times over.
        out = args.out_dir / f"{version}.bin"
        out.write_bytes(b"".join(value.to_bytes(2, "little") for value in table))
        total = sum(counts.values())
        print(
            f"{version:>8}: {total} states -> {counts['exact']} exact, "
            f"{counts['default_state']} default state, {counts['family']} same family, "
            f"{counts['stone']} stone"
        )


if __name__ == "__main__":
    main()
