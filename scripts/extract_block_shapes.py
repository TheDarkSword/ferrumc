#!/usr/bin/env python3
"""Extract the block data that lives in code rather than in the data generator's reports.

Collision shapes, light emission, hardness and the rest are built by the blocks themselves,
sometimes from lambdas over their own property values, so no report carries them and the game has
to be asked. Since 26.1 the server jar ships with its own names, so `scripts/extractor/` compiles
straight against it with nothing in between.

Output is `assets/extracted/<version>/block_shapes.json`: distinct boxes, distinct shapes as lists
of box indices, and one entry per block state id.

Usage:
    scripts/extract_block_shapes.py
    scripts/extract_block_shapes.py 26.1
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from extract_assets import DEFAULT_CACHE, download_server_jar  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parent.parent
EXTRACTOR = REPO_ROOT / "scripts" / "extractor" / "BlockShapeExtractor.java"
# The version the server itself speaks.
NATIVE = "26.2"
# Older jars are obfuscated, so the extractor would not compile against them.
OLDEST_NAMED = "26.1"


def unpack(jar: Path, work: Path) -> tuple[Path, list[Path]]:
    """Pull the real server jar and its libraries out of the bundler."""
    with zipfile.ZipFile(jar) as archive:
        members = [
            name
            for name in archive.namelist()
            if name.startswith(("META-INF/versions/", "META-INF/libraries/"))
            and name.endswith(".jar")
        ]
        archive.extractall(work, members)

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
    out = REPO_ROOT / "assets" / "extracted" / args.version / "block_shapes.json"
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
    print(
        f"{len(data['states'])} states, {len(data['shapes'])} shapes, {len(data['boxes'])} boxes "
        f"-> {out.relative_to(REPO_ROOT)}"
    )


if __name__ == "__main__":
    main()
