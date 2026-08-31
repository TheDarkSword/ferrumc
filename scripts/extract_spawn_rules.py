#!/usr/bin/env python3
"""Ask the game which rule decides whether a mob may appear at a place.

Where a mob may stand and which heightmap it stands on are on the type and come out of
`extract_entity_types.py`. What is left is the condition: how dark it has to be, what it has to
stand on, how deep the water must be. Vanilla holds that as a method reference per type, so no
report carries it and no instance can be asked for it — but the method it points at is named in the
bootstrap arguments of the lambda, which is plain to read.

Nine or so conditions cover half the types, and the rest are one mob each. What is written here is
the name of the method, so the ones that are not modelled yet say so rather than quietly behaving
like something else.

Output is `assets/extracted/<version>/spawn_rules.json`.

Usage:
    scripts/extract_spawn_rules.py
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from extract_field_layouts import ENTITY_TYPE, ENTITY_TYPES, Jar, bootstrap_targets, clinit  # noqa: E402
from extract_serializer_ids import DEFAULT_CACHE  # noqa: E402

PLACEMENTS = "net.minecraft.world.entity.SpawnPlacements"
NATIVE = "26.2"


def bootstrap_methods(listing: str) -> dict[int, str]:
    """Which method each lambda in a class stands for.

    Unlike a factory, a rule is a plain static method rather than a constructor, so the handle is
    an invocation rather than an allocation.
    """
    section = listing.find("BootstrapMethods:")
    if section < 0:
        return {}
    found: dict[int, str] = {}
    index = None
    for line in listing[section:].splitlines():
        header = re.match(r"\s+(\d+): #\d+ ", line)
        if header:
            index = int(header.group(1))
        called = re.search(r"REF_invoke\w+ ([\w/$]+)\.(\w+):", line)
        if called and index is not None:
            found[index] = f"{called.group(1).rsplit('/', 1)[-1]}.{called.group(2)}"
    return found


def rules(version: str, cache: Path) -> dict[str, str]:
    with tempfile.TemporaryDirectory() as name:
        jar = Jar(version, Path(name), cache)
        listing = jar.listing(PLACEMENTS)
        if not listing:
            raise SystemExit(f"{version} has no {PLACEMENTS}")
        lambdas = {**bootstrap_targets(listing), **bootstrap_methods(listing)}
        holder = jar.path_of(ENTITY_TYPES if jar.listing(ENTITY_TYPES) else ENTITY_TYPE)
        held = jar.path_of(ENTITY_TYPE)

        found: dict[str, str] = {}
        subject: str | None = None
        for line in clinit(listing).splitlines():
            named = re.search(rf"getstatic .*// Field {re.escape(holder)}\.(\w+):L{re.escape(held)};", line)
            if named:
                subject = named.group(1).lower()
            indy = re.search(r"invokedynamic #\d+,\s+\d+\s+// InvokeDynamic #(\d+):", line)
            if indy and subject is not None:
                rule = lambdas.get(int(indy.group(1)))
                if rule is not None:
                    found[f"minecraft:{subject}"] = rule
                subject = None
    return found


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", nargs="?", default=NATIVE)
    parser.add_argument("--cache", type=Path, default=DEFAULT_CACHE)
    args = parser.parse_args()

    found = rules(args.version, args.cache)
    out = REPO_ROOT / "assets" / "extracted" / args.version / "spawn_rules.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(
        json.dumps({"version": args.version, "rules": dict(sorted(found.items()))}, indent=1) + "\n"
    )
    distinct = len(set(found.values()))
    print(f"{len(found)} types, {distinct} distinct rules -> {out.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()
