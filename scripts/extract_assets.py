#!/usr/bin/env python3
"""Extract vanilla data for a Minecraft version into assets/extracted/<version>/.

Downloads the official server jar, verifies it against the checksum Mojang
publishes, and runs the jar's own data generator. Output is the raw generator
result: `reports/` (packets, blocks, registries, commands, biome parameters)
and `data/` (the built-in datapack: loot tables, recipes, advancements, tags,
worldgen definitions).

Nothing here is Minecraft source code, so the result is safe to commit; only
the reference decompilation under .vanilla-reference/ is not.

Usage:
    scripts/extract_assets.py 26.2
    scripts/extract_assets.py 1.21.8 --out-dir /tmp/compare
    scripts/extract_assets.py --list
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import zipfile
from pathlib import Path

MANIFEST_URL = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
DATA_GENERATOR_MAIN = "net.minecraft.data.Main"
REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUT_ROOT = REPO_ROOT / "assets" / "extracted"
DEFAULT_CACHE = Path.home() / ".cache" / "ferrumc-mc-jars"


def fetch_json(url: str) -> dict:
    with urllib.request.urlopen(url, timeout=60) as response:
        return json.load(response)


def sha1_of(path: Path) -> str:
    digest = hashlib.sha1()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def download_server_jar(version: str, cache_dir: Path) -> Path:
    """Return the cached server jar for `version`, downloading it if needed."""
    manifest = fetch_json(MANIFEST_URL)
    entry = next((v for v in manifest["versions"] if v["id"] == version), None)
    if entry is None:
        raise SystemExit(
            f"unknown version {version!r}; run with --list to see what Mojang publishes"
        )

    server = fetch_json(entry["url"])["downloads"].get("server")
    if server is None:
        raise SystemExit(f"version {version} publishes no server jar")

    cache_dir.mkdir(parents=True, exist_ok=True)
    jar = cache_dir / f"server-{version}.jar"
    if jar.exists() and sha1_of(jar) == server["sha1"]:
        print(f"using cached {jar}")
        return jar

    print(f"downloading server jar for {version} ({server['size'] / 1e6:.0f} MB)")
    urllib.request.urlretrieve(server["url"], jar)

    actual = sha1_of(jar)
    if actual != server["sha1"]:
        jar.unlink()
        raise SystemExit(
            f"checksum mismatch for {version}: expected {server['sha1']}, got {actual}"
        )
    return jar


def run_data_generator(jar: Path, work_dir: Path, java: str) -> Path:
    """Run the jar's data generator and return the directory it wrote."""
    generated = work_dir / "generated"
    # Server jars have been bundlers since 1.18, so the generator entry point is
    # selected through the bundler rather than by running the class directly.
    command = [
        java,
        f"-DbundlerMainClass={DATA_GENERATOR_MAIN}",
        "-jar",
        str(jar),
        "--server",
        "--reports",
        "--output",
        str(generated),
    ]
    print(f"running data generator: {' '.join(command)}")
    result = subprocess.run(command, cwd=work_dir, capture_output=True, text=True)
    if result.returncode != 0:
        sys.stderr.write(result.stdout[-4000:])
        sys.stderr.write(result.stderr[-4000:])
        raise SystemExit(f"data generator failed with exit code {result.returncode}")
    if not generated.is_dir():
        raise SystemExit(f"data generator produced no output at {generated}")
    return generated


def extract_version_metadata(jar: Path, out_dir: Path) -> dict:
    """Copy version.json out of the jar; it carries the protocol number."""
    with zipfile.ZipFile(jar) as archive:
        try:
            raw = archive.read("version.json")
        except KeyError:
            print("warning: jar has no version.json, skipping metadata")
            return {}
    (out_dir / "version.json").write_bytes(raw)
    return json.loads(raw)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", nargs="?", help="version id, e.g. 26.2")
    parser.add_argument("--list", action="store_true", help="list released versions")
    parser.add_argument("--out-dir", type=Path, help="override the output directory")
    parser.add_argument("--cache-dir", type=Path, default=DEFAULT_CACHE)
    parser.add_argument("--java", default="java", help="java binary to run the generator")
    args = parser.parse_args()

    if args.list:
        manifest = fetch_json(MANIFEST_URL)
        for entry in manifest["versions"]:
            if entry["type"] == "release":
                print(entry["id"])
        return

    if not args.version:
        parser.error("a version is required unless --list is given")

    out_dir = args.out_dir or DEFAULT_OUT_ROOT / args.version
    jar = download_server_jar(args.version, args.cache_dir)

    with tempfile.TemporaryDirectory(prefix="ferrumc-extract-") as tmp:
        generated = run_data_generator(jar, Path(tmp), args.java)

        if out_dir.exists():
            shutil.rmtree(out_dir)
        out_dir.mkdir(parents=True)
        for name in ("reports", "data"):
            source = generated / name
            if source.is_dir():
                shutil.copytree(source, out_dir / name)
            else:
                print(f"warning: generator produced no {name}/")

    metadata = extract_version_metadata(jar, out_dir)
    if metadata:
        print(
            f"{metadata.get('id')}: protocol {metadata.get('protocol_version')}, "
            f"world version {metadata.get('world_version')}"
        )
    print(f"wrote {out_dir}")


if __name__ == "__main__":
    main()
