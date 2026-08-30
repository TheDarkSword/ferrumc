#!/usr/bin/env python3
"""Rebuild the files under assets/data/ that are derived from one version's vanilla data.

These feed compile-time lookups - the `block!` and `get_registry_entry!` macros, the registry
crate's perfect hash tables, the world importer - so a file left behind from an older version is
not a missing feature but a wrong answer, and a silent one: the server still runs and clients still
connect, they just see different blocks and entities than the ones that are there.

`blockstates.json` is written by `scripts/build_block_states.py`, which owns the block state tables
as a whole.

Usage:
    scripts/sync_data_assets.py
    scripts/sync_data_assets.py --check
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
# The version the server itself speaks.
NATIVE = "26.2"
REPORTS = REPO_ROOT / "assets" / "extracted" / NATIVE / "reports"
DATA = REPO_ROOT / "assets" / "data"


def item_to_block(registries: dict, blocks: dict) -> dict[str, str]:
    """Which block state placing each item produces.

    An item is placeable when the block registry has a block of the same name, which covers
    everything but the handful that place something else entirely.
    """
    items = registries["minecraft:item"]["entries"]
    defaults = {
        name: next(state["id"] for state in block["states"] if state.get("default"))
        for name, block in blocks.items()
    }
    return {
        str(entry["protocol_id"]): str(defaults[name])
        for name, entry in items.items()
        if name in defaults
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="report what is out of date without writing anything",
    )
    args = parser.parse_args()

    with (REPORTS / "registries.json").open() as handle:
        registries = json.load(handle)
    with (REPORTS / "blocks.json").open() as handle:
        blocks = json.load(handle)
    with (REPORTS / "packets.json").open() as handle:
        packets = json.load(handle)

    wanted = {
        "registries.json": registries,
        "packets.json": packets,
        "item_to_block_mapping.json": item_to_block(registries, blocks),
    }

    stale = []
    for name, content in wanted.items():
        path = DATA / name
        current = None
        if path.exists():
            with path.open() as handle:
                current = json.load(handle)
        if current == content:
            print(f"{name}: up to date")
            continue
        stale.append(name)
        if args.check:
            print(f"{name}: STALE")
            continue
        path.write_text(json.dumps(content, separators=(",", ":"), sort_keys=True))
        print(f"{name}: rewritten from {NATIVE}")

    if args.check and stale:
        sys.exit(1)


if __name__ == "__main__":
    main()
