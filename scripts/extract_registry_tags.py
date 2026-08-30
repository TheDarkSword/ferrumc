#!/usr/bin/env python3
"""Ask the game what NBT tag each field of a synced registry entry carries.

The registry payload a client is sent is NBT built from json. Json has one number type and NBT has
six, so without knowing better every integer goes out as a `Long` and every real as a `Double`. A
lenient client coerces them; a strict one refuses a field whose tag is not what its schema says.

There is no rule to guess by: most numeric fields in these registries are `Float`, a few are `Int`,
and they are declared in types far from the registry itself. So each entry is read through its own
codec and written back out as NBT, and the tag of every field is recorded.

Output is `assets/data/registry_tags/<version>.json`.

Usage:
    scripts/extract_registry_tags.py
    scripts/extract_registry_tags.py 26.1
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from extract_assets import DEFAULT_CACHE, download_server_jar  # noqa: E402

EXTRACTOR = REPO_ROOT / "scripts" / "extractor" / "RegistryTagExtractor.java"
NATIVE = "26.2"
# Older jars are obfuscated, so the extractor would not compile against them.
OLDEST_NAMED = "26.1"


def unpack(jar: Path, work: Path) -> tuple[Path, list[Path]]:
    """Pull the real server jar and its libraries out of the bundler."""
    with zipfile.ZipFile(jar) as archive:
        archive.extractall(work)
    versions = sorted((work / "META-INF" / "versions").rglob("*.jar"))
    if not versions:
        raise SystemExit(f"{jar} carries no server jar")
    libraries = sorted((work / "META-INF" / "libraries").rglob("*.jar"))
    return versions[0], libraries


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", nargs="?", default=NATIVE)
    parser.add_argument("--java", default="java")
    parser.add_argument("--javac", default="javac")
    args = parser.parse_args()

    if args.version < OLDEST_NAMED:
        raise SystemExit(
            f"{args.version} ships an obfuscated jar; the extractor needs {OLDEST_NAMED} or newer"
        )

    payload = REPO_ROOT / "assets" / "data" / "registry_packets" / f"{args.version}.json"
    if not payload.exists():
        raise SystemExit(f"no registry payload for {args.version} at {payload}")

    jar = download_server_jar(args.version, DEFAULT_CACHE)
    out = REPO_ROOT / "assets" / "data" / "registry_tags" / f"{args.version}.json"
    out.parent.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory() as name:
        work = Path(name)
        server, libraries = unpack(jar, work)
        classpath = ":".join(str(path) for path in [server, *libraries])
        classes = work / "classes"
        classes.mkdir()

        subprocess.run(
            [args.javac, "-nowarn", "-cp", classpath, "-d", str(classes), str(EXTRACTOR)],
            check=True,
        )
        subprocess.run(
            [args.java, "-cp", f"{classes}:{classpath}", EXTRACTOR.stem, str(payload), str(out)],
            check=True,
        )

    with out.open() as handle:
        data = json.load(handle)
    fields = sum(len(fields) for fields in data["registries"].values())
    print(
        f"{len(data['registries'])} registries, {fields} fields -> {out.relative_to(REPO_ROOT)}"
    )
    if data["unread"]:
        print(f"{len(data['unread'])} entries could not be read:")
        for line in data["unread"][:10]:
            print(f"  {line}")


if __name__ == "__main__":
    main()
