#!/usr/bin/env python3
"""Build assets/data/registry_remap/<registry>/<version>.bin.

Registry ids are dense and assigned in name order, so one added item shifts every id after it. A
client sent an id from a newer registry reads whatever now sits at that index. Unlike block states
these registries carry no properties, so the table is a plain name lookup: the id a name has in the
server's own version maps to the id the same name has in an older one.

Names an older version does not have are the interesting case, and what to do about them differs
per registry:

- items get a stand-in from the same family, found from the longest shared name suffix, because a
  container slot has to hold something;
- everything else gets `NO_EQUIVALENT`, leaving the decision to the sending code, which drops the
  packet rather than showing the wrong entity or playing the wrong sound.

Usage:
    scripts/build_registry_remap.py 1.21.4
    scripts/build_registry_remap.py --all

Output is a flat little-endian u16 array indexed by the server's own id.
"""

from __future__ import annotations

import argparse
import json
import struct
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
EXTRACTED = REPO_ROOT / "assets" / "extracted"
OUT_ROOT = REPO_ROOT / "assets" / "data" / "registry_remap"

# The version the server itself speaks; every table maps from it.
NATIVE = "26.2"

# No id in the target version means the same thing. Callers drop what carries it.
NO_EQUIVALENT = 0xFFFF

# Registries that travel on the wire as bare ids, and whether a missing name may be substituted.
REGISTRIES = {
    "minecraft:item": {"dir": "item", "substitute": True},
    "minecraft:entity_type": {"dir": "entity_type", "substitute": False},
    "minecraft:sound_event": {"dir": "sound_event", "substitute": False},
    "minecraft:particle_type": {"dir": "particle_type", "substitute": False},
}


def segments(name: str) -> list[str]:
    """A name split into the words that carry its meaning, namespace dropped."""
    return name.split(":", 1)[-1].split("_")


def shared_suffix(a: list[str], b: list[str]) -> int:
    """How many trailing words two names share."""
    count = 0
    while count < len(a) and count < len(b) and a[-1 - count] == b[-1 - count]:
        count += 1
    return count


def substitute(name: str, available: dict[str, int]) -> str | None:
    """Pick a name in `available` to stand in for one the target version lacks.

    The last word of a name is what the thing is - a `copper_pickaxe` is a pickaxe, a
    `pale_oak_trapdoor` is a trapdoor - and the words before it are how it differs. Matching whole
    trailing words is what keeps families apart: by characters a `music_disc_lava_chicken` shares
    "chicken" with `cooked_chicken` and becomes food.

    Among equals the shortest name wins, which prefers the plain member of a family over a
    decorated one: a `waxed_copper_lantern` becomes a `lantern` rather than a `jack_o_lantern`.

    Names whose family sits at the front rather than the back are where this is wrong - a music
    disc is a disc, not the thing it is named after - and a stand-in for one of those is a wrong
    icon in a slot. Nothing carries further than the icon: the substitution is only ever an id.
    """
    words = segments(name)
    best, best_key = None, None
    for candidate in available:
        other = segments(candidate)
        score = shared_suffix(words, other)
        if score == 0:
            continue
        key = (score, -len(candidate))
        if best_key is None or key > best_key:
            best, best_key = candidate, key
    return best


def entries(version: str, registry: str) -> dict[str, int]:
    report = EXTRACTED / version / "reports" / "registries.json"
    with report.open() as handle:
        data = json.load(handle)
    if registry not in data:
        return {}
    return {name: value["protocol_id"] for name, value in data[registry]["entries"].items()}


def build(version: str) -> None:
    for registry, options in REGISTRIES.items():
        native = entries(NATIVE, registry)
        target = entries(version, registry)
        if not native:
            raise SystemExit(f"{NATIVE} has no {registry}")

        table = [NO_EQUIVALENT] * (max(native.values()) + 1)
        missing = substituted = 0
        for name, native_id in native.items():
            if name in target:
                table[native_id] = target[name]
                continue
            stand_in = substitute(name, target) if options["substitute"] else None
            if stand_in is None:
                missing += 1
            else:
                table[native_id] = target[stand_in]
                substituted += 1

        out_dir = OUT_ROOT / options["dir"]
        out_dir.mkdir(parents=True, exist_ok=True)
        out = out_dir / f"{version}.bin"
        out.write_bytes(struct.pack(f"<{len(table)}H", *table))
        print(
            f"{version} {registry}: {len(table)} ids, "
            f"{substituted} substituted, {missing} without an equivalent -> {out.name}"
        )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("versions", nargs="*", help="versions to build, e.g. 1.21.4")
    parser.add_argument("--all", action="store_true", help="build every extracted version")
    args = parser.parse_args()

    versions = args.versions
    if args.all:
        versions = sorted(path.name for path in EXTRACTED.iterdir() if path.is_dir())
    if not versions:
        parser.error("give a version or --all")

    for version in versions:
        build(version)


if __name__ == "__main__":
    main()
