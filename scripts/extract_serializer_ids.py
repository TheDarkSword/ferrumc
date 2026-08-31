#!/usr/bin/env python3
"""Ask an obfuscated jar what number each kind of synced value travels as.

A synced field is written as an index, a number saying what kind of value follows, and the value.
That number is the order the serializers were registered in, and it moves between versions: what a
26.2 client reads as a pose, a 1.21.8 client reads as an optional unsigned int, and since the kind
decides how many bytes to read, one wrong number desynchronises everything after it.

`extract_synched_data.py` asks the game directly, which only works from 26.1 on because older jars
are obfuscated. This asks the same question of any version by reading the class file instead of
running it: Mojang publishes a mapping file for every release since 1.14.4, which says what
`EntityDataSerializers` was renamed to and what each of its fields was renamed to, and the
registration order is plain to see in its static initialiser.

Nothing is executed and nothing is remapped; `javap` is the only tool.

Output is `assets/extracted/<version>/serializer_ids.json`.

Usage:
    scripts/extract_serializer_ids.py --all
    scripts/extract_serializer_ids.py 1.21.8
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
import urllib.request
import zipfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from extract_assets import DEFAULT_CACHE, download_server_jar  # noqa: E402

MANIFEST = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json"

# The versions the server speaks, oldest first. Has to match
# `ferrumc_net_codec::version::ProtocolVersion::ALL`.
VERSIONS = [
    "1.21",
    "1.21.2",
    "1.21.4",
    "1.21.5",
    "1.21.6",
    "1.21.8",
    "1.21.9",
    "1.21.11",
    "26.1",
    "26.2",
]

SERIALIZERS = "net.minecraft.network.syncher.EntityDataSerializers"
SERIALIZER = "net.minecraft.network.syncher.EntityDataSerializer"
REGISTER = "registerSerializer"


def mappings_for(version: str, cache: Path) -> Path | None:
    """Mojang's own mapping file for a version."""
    path = cache / f"mappings-{version}.txt"
    if path.exists():
        return path

    with urllib.request.urlopen(MANIFEST) as handle:
        manifest = json.load(handle)
    entry = next((v for v in manifest["versions"] if v["id"] == version), None)
    if entry is None:
        raise SystemExit(f"the manifest has no {version}")
    with urllib.request.urlopen(entry["url"]) as handle:
        downloads = json.load(handle)["downloads"]
    if "server_mappings" not in downloads:
        return None

    cache.mkdir(parents=True, exist_ok=True)
    with urllib.request.urlopen(downloads["server_mappings"]["url"]) as handle:
        path.write_bytes(handle.read())
    return path


class Renames:
    """What a mapping file says a name was turned into, and back again."""

    def __init__(self, path: Path | None) -> None:
        self.classes: dict[str, str] = {}
        # Fields are looked up by the name in the jar and methods by the name in the source, so
        # they are kept apart: a class can rename a field and a method to the same letter, and an
        # overloaded method renames every one of its forms to the same letter as well.
        self.fields: dict[str, dict[str, str]] = {}
        self.methods: dict[str, dict[str, str]] = {}
        if path is None:
            return

        owner = None
        for line in path.read_text().splitlines():
            if line.startswith("#"):
                # A comment sits inside the class it describes and does not end it.
                continue
            if not line.startswith(" "):
                match = re.match(r"([\w.$]+) -> ([\w.$]+):$", line)
                owner = match.group(1) if match else None
                if match:
                    self.classes[match.group(1)] = match.group(2)
                continue
            if owner is None:
                continue
            method = re.match(r"\s+(?:\d+:\d+:)?\S+ (\w+)\([^)]*\) -> (\w+)$", line)
            if method:
                self.methods.setdefault(owner, {})[method.group(1)] = method.group(2)
                continue
            field = re.match(r"\s+\S+ (\w+) -> (\w+)$", line)
            if field:
                self.fields.setdefault(owner, {})[field.group(2)] = field.group(1)

    def obfuscated(self, name: str) -> str:
        """What a class is called in the jar."""
        return self.classes.get(name, name)

    def real_field(self, owner: str, obfuscated: str) -> str:
        """What a field of a class was called before it was renamed."""
        return self.fields.get(owner, {}).get(obfuscated, obfuscated)

    def obfuscated_method(self, owner: str, name: str) -> str:
        """What a method of a class is called in the jar."""
        return self.methods.get(owner, {}).get(name, name)


def server_jar(bundle: Path, work: Path) -> Path:
    """The real server jar, out of the bundler that ships around it."""
    with zipfile.ZipFile(bundle) as archive:
        archive.extractall(work, [n for n in archive.namelist() if n.startswith("META-INF/versions/")])
    versions = sorted((work / "META-INF" / "versions").rglob("*.jar"))
    if not versions:
        # Before the bundler, the downloaded jar is the server jar.
        return bundle
    return versions[0]


def registration_order(version: str, cache: Path) -> list[str]:
    """The kinds of value in the order they were registered, which is the order they travel as."""
    bundle = download_server_jar(version, cache)
    with tempfile.TemporaryDirectory() as name:
        work = Path(name)
        jar = server_jar(bundle, work)
        with zipfile.ZipFile(jar) as archive:
            held = set(archive.namelist())
            # Since 26.1 the jar carries its own names, so there is nothing to look up.
            named = f"{SERIALIZERS.replace('.', '/')}.class" in held
            renames = Renames(None if named else mappings_for(version, cache))
            holder = renames.obfuscated(SERIALIZERS)
            field_type = renames.obfuscated(SERIALIZER)
            register = renames.obfuscated_method(SERIALIZERS, REGISTER)
            archive.extract(f"{holder.replace('.', '/')}.class", work)
        holder = holder.replace(".", "/")
        listing = subprocess.run(
            ["javap", "-p", "-c", str(work / f"{holder}.class")],
            capture_output=True,
            text=True,
            check=True,
        ).stdout

    body = listing[listing.index("static {}") :]
    # Each registration is the field being pushed and then handed to the register method, so the
    # last field seen before a call is the one that call registers.
    holds = re.compile(rf"getstatic .*// Field (\w+):L{re.escape(field_type.replace('.', '/'))};")
    calls = re.compile(rf"invokestatic .*// Method {re.escape(register)}:")
    order: list[str] = []
    pending: str | None = None
    for line in body.splitlines():
        held = holds.search(line)
        if held:
            pending = held.group(1)
        if calls.search(line) and pending is not None:
            order.append(renames.real_field(SERIALIZERS, pending).lower())
            pending = None

    if not order:
        raise SystemExit(f"{version}: nothing looked like a registration in {holder}")
    return order


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", nargs="?")
    parser.add_argument("--all", action="store_true")
    parser.add_argument("--cache", type=Path, default=DEFAULT_CACHE)
    args = parser.parse_args()

    wanted = VERSIONS if args.all else [args.version or VERSIONS[-1]]
    for version in wanted:
        order = registration_order(version, args.cache)
        out = REPO_ROOT / "assets" / "extracted" / version / "serializer_ids.json"
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps({"version": version, "serializers": order}, indent=1) + "\n")
        print(f"{version}: {len(order)} kinds -> {out.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()
