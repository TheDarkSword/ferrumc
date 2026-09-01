#!/usr/bin/env python3
"""Ask the game which attributes each kind of entity starts with.

A zombie has twenty health, a movement speed of 0.23 and a follow range of 35. None of that is in
any report: it is built in code by each entity class rather than written in data.

Output is `assets/extracted/default_attributes.json`.

Usage:
    scripts/extract_default_attributes.py
    scripts/extract_default_attributes.py 26.1
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

EXTRACTOR = REPO_ROOT / "scripts" / "extractor" / "DefaultAttributeExtractor.java"
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

    jar = download_server_jar(args.version, DEFAULT_CACHE)
    out = REPO_ROOT / "assets" / "extracted" / "default_attributes.json"
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
            [args.java, "-cp", f"{classes}:{classpath}", EXTRACTOR.stem, str(out)],
            check=True,
        )

    with out.open() as handle:
        data = json.load(handle)
    print(f"{len(data)} kinds -> {out.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()
