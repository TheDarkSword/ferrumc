#!/usr/bin/env python3
"""Build assets/data/registry_packets.json from an extracted vanilla datapack.

The configuration state sends the client the contents of every synchronized
registry. `ferrumc_macros::build_registry_packets!` bakes that JSON into the
binary, preserving order, so entries must appear in the same order vanilla
registers them: sorted by resource location.

Usage:
    scripts/build_registry_packets.py assets/extracted/26.2
    scripts/build_registry_packets.py assets/extracted/1.21.8 --out /tmp/check.json
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUT = REPO_ROOT / "assets" / "data" / "registry_packets.json"

# Registries the server synchronizes to the client, taken from
# RegistryDataLoader.SYNCHRONIZED_REGISTRIES. Datapack directories that are not
# synchronized (advancements, recipes, loot tables, tags) are deliberately absent.
# A registry missing from an older version's datapack is skipped with a warning.
SYNCHRONIZED_REGISTRIES = [
    "banner_pattern",
    "cat_sound_variant",
    "cat_variant",
    "chat_type",
    "chicken_sound_variant",
    "chicken_variant",
    "cow_sound_variant",
    "cow_variant",
    "damage_type",
    "dialog",
    "dimension_type",
    "enchantment",
    "frog_variant",
    "instrument",
    "jukebox_song",
    "painting_variant",
    "pig_sound_variant",
    "pig_variant",
    "sulfur_cube_archetype",
    "test_environment",
    "test_instance",
    "timeline",
    "trim_material",
    "trim_pattern",
    "wolf_sound_variant",
    "wolf_variant",
    "world_clock",
    "worldgen/biome",
    "zombie_nautilus_variant",
]


# Fields a registry keeps on disk but does not put on the wire, taken from the
# gap between each type's DIRECT_CODEC and its NETWORK_CODEC. Registries absent
# from this table have no NETWORK_CODEC and are sent whole.
SERVER_ONLY_FIELDS = {
    # Biome.NETWORK_CODEC keeps only climate, attributes and effects, dropping
    # BiomeGenerationSettings and MobSpawnSettings entirely.
    "worldgen/biome": {
        "carvers",
        "creature_spawn_probability",
        "features",
        "spawn_costs",
        "spawners",
    },
    # Every animal variant drops the rules that decide where it spawns.
    "cat_variant": {"spawn_conditions"},
    "chicken_variant": {"spawn_conditions"},
    "cow_variant": {"spawn_conditions"},
    "frog_variant": {"spawn_conditions"},
    "pig_variant": {"spawn_conditions"},
    "wolf_variant": {"spawn_conditions"},
    "zombie_nautilus_variant": {"spawn_conditions"},
}


def collect_registry(directory: Path, server_only: set[str]) -> dict:
    """Read one registry directory into name -> wire contents, ordered by name."""
    entries = {}
    for path in sorted(directory.rglob("*.json")):
        name = path.relative_to(directory).with_suffix("").as_posix()
        entry = json.loads(path.read_text(encoding="utf-8"))
        if server_only and isinstance(entry, dict):
            entry = {k: v for k, v in entry.items() if k not in server_only}
        entries[name] = entry
    return entries


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "extracted", type=Path, help="an assets/extracted/<version> directory"
    )
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    parser.add_argument(
        "--namespace", default="minecraft", help="datapack namespace to read"
    )
    args = parser.parse_args()

    root = args.extracted / "data" / args.namespace
    if not root.is_dir():
        raise SystemExit(f"no datapack at {root}")

    registries = {}
    for registry in SYNCHRONIZED_REGISTRIES:
        directory = root / registry
        if not directory.is_dir():
            print(f"warning: {registry} absent from this version, skipping")
            continue
        entries = collect_registry(directory, SERVER_ONLY_FIELDS.get(registry, set()))
        if entries:
            registries[f"{args.namespace}:{registry}"] = entries

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(registries, indent=2) + "\n", encoding="utf-8")

    total = sum(len(v) for v in registries.values())
    print(f"wrote {args.out}: {len(registries)} registries, {total} entries")


if __name__ == "__main__":
    main()
