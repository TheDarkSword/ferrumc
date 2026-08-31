#!/usr/bin/env python3
"""Ask any jar where each of an entity's synced fields sits.

Vanilla hands out a field's index by walking the entity class tree: a class's fields are numbered
from one past its superclass's last, so where a field sits depends on every class above it. Nothing
in any report says so, and `extract_synched_data.py` can only ask the versions whose jars carry
their own names.

This asks the rest, by reading the class files rather than running them. Three things are in the
bytecode and nothing else is needed:

- which class each entity type is built from, named in the bootstrap arguments of the lambda behind
  its factory;
- which class each class extends;
- and each class's own fields, in order, as the `defineId` calls in its static initialiser.

Mojang publishes a mapping file for every release since 1.14.4, so the same reading works on a jar
that carries no names. Nothing is executed and nothing is remapped; `javap` is the only tool.

Output is `assets/extracted/<version>/field_layouts.json`.

Usage:
    scripts/extract_field_layouts.py --all
    scripts/extract_field_layouts.py 1.21.8
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import zipfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from extract_assets import DEFAULT_CACHE, download_server_jar  # noqa: E402
from extract_serializer_ids import VERSIONS, Renames, mappings_for, server_jar  # noqa: E402

ENTITY = "net.minecraft.world.entity.Entity"
ENTITY_TYPE = "net.minecraft.world.entity.EntityType"
# Since 26.1 the types themselves live beside the class that describes one.
ENTITY_TYPES = "net.minecraft.world.entity.EntityTypes"
FACTORY = "net.minecraft.world.entity.EntityType$EntityFactory"
SYNCHED = "net.minecraft.network.syncher.SynchedEntityData"
SERIALIZERS = "net.minecraft.network.syncher.EntityDataSerializers"
DEFINE = "defineId"

# How many class files to hand javap at once. One call per class spends more time starting a JVM
# than reading anything.
BATCH = 60


class Jar:
    """An unpacked server jar, read through whatever names it happens to carry."""

    def __init__(self, version: str, work: Path, cache: Path) -> None:
        bundle = download_server_jar(version, cache)
        self.work = work
        self.archive = zipfile.ZipFile(server_jar(bundle, work))
        held = set(self.archive.namelist())
        named = f"{ENTITY.replace('.', '/')}.class" in held
        self.renames = Renames(None if named else mappings_for(version, cache))
        self.back = {short: long for long, short in self.renames.classes.items()}
        self._disassembled: dict[str, str] = {}

    def path_of(self, name: str) -> str:
        return self.renames.obfuscated(name).replace(".", "/")

    def real(self, path: str) -> str:
        """The name a class had before it was renamed, given the name it has in the jar."""
        dotted = path.replace("/", ".")
        return self.back.get(dotted, dotted)

    def disassemble(self, names: list[str]) -> None:
        """Reads a batch of classes, keeping what comes back under their real names.

        Always the full reading: it is the only one that prints the path of each class it read, and
        that path is the only reliable way to tell one class's output from the next's.
        """
        wanted = [n for n in names if n not in self._disassembled]
        for start in range(0, len(wanted), BATCH):
            batch = wanted[start : start + BATCH]
            files = []
            for name in batch:
                member = f"{self.path_of(name)}.class"
                try:
                    files.append(self.archive.extract(member, self.work))
                except KeyError:
                    self._disassembled[name] = ""
            if not files:
                continue
            listing = subprocess.run(
                ["javap", "-p", "-c", "-v", *files], capture_output=True, text=True, check=True
            ).stdout
            # javap runs the classes together; each starts with its own compilation banner.
            for chunk in listing.split("Classfile ")[1:]:
                path = chunk.split("\n", 1)[0].strip()
                stem = Path(path).with_suffix("").as_posix()
                for name in batch:
                    if stem.endswith(self.path_of(name)):
                        self._disassembled[name] = chunk
                        break

    def listing(self, name: str) -> str:
        self.disassemble([name])
        return self._disassembled.get(name, "")


def clinit(listing: str) -> str:
    """Just the static initialiser, which is where everything interesting happens."""
    marker = listing.find("static {}")
    return "" if marker < 0 else listing[marker:]


# What the game says it builds nothing for, because the server builds it itself. The bytecode
# names no class for these, which is the truth rather than a gap; the layout test checks the answer
# against the game's own for the versions that can be asked directly.
BUILDS_ITSELF = {
    "minecraft:player": "net.minecraft.world.entity.player.Player",
    "minecraft:mannequin": "net.minecraft.world.entity.decoration.Mannequin",
}


def bootstrap_targets(listing: str) -> dict[int, str]:
    """Which constructor each lambda in a class stands for."""
    section = listing.find("BootstrapMethods:")
    if section < 0:
        return {}
    targets: dict[int, str] = {}
    index = None
    for line in listing[section:].splitlines():
        header = re.match(r"\s+(\d+): #\d+ ", line)
        if header:
            index = int(header.group(1))
        made = re.search(r"REF_newInvokeSpecial ([\w/$]+)\.\"<init>\"", line)
        if made and index is not None:
            targets[index] = made.group(1)
    return targets


def helper_factories(jar: "Jar", listing: str) -> dict[str, str]:
    """The little methods that hand back a factory, and the class each one builds.

    A boat is not registered with a constructor but with a helper that wraps one, so the class is a
    hop further away than for everything else. The helper does nothing but make the lambda, so its
    single bootstrap entry names the class.
    """
    # The class is in the type the helper hands back, which survives obfuscation because a
    # generic signature is kept. Going after the lambda instead would mean two more hops: one that
    # captures an argument is compiled to a synthetic method, with the constructor inside that.
    factory = jar.renames.obfuscated(FACTORY)
    hands_back = re.compile(rf"{re.escape(factory)}<([\w.$]+)>")

    made: dict[str, str] = {}
    for chunk in re.split(r"\n  (?=\S)", listing):
        signature = chunk.split("\n", 1)[0]
        name = re.search(r"\b(\w+)\(", signature)
        builds = hands_back.search(signature)
        if name and builds:
            made[name.group(1)] = builds.group(1)
    return made


def implementations(jar: Jar) -> dict[str, str]:
    """Which class each entity type is built from, by the name the registry gives the type."""
    holder = ENTITY_TYPES if jar.listing(ENTITY_TYPES) else ENTITY_TYPE
    listing = jar.listing(holder)
    targets = bootstrap_targets(listing)
    helpers = helper_factories(jar, listing)
    type_class = jar.path_of(ENTITY_TYPE)

    # A call is only a factory when it hands back a factory. Matching on the name alone is enough
    # on a jar that carries its own names and hopeless on one that does not, where every other
    # method is called `a`.
    factory = jar.path_of(FACTORY)
    makes_one = re.compile(rf"invokedynamic .*// InvokeDynamic #(\d+):\w+:\([^)]*\)L{re.escape(factory)};")
    calls_one = re.compile(rf"invokestatic .*// Method (\w+):\([^)]*\)L{re.escape(factory)};")

    found: dict[str, str] = {}
    built: str | None = None
    literal: str | None = None
    for line in clinit(listing).splitlines():
        text = re.search(r"ldc\w*\s+#\d+\s+// String (\S+)", line)
        if text:
            literal = text.group(1)
        indy = makes_one.search(line)
        if indy and int(indy.group(1)) in targets:
            built = targets[int(indy.group(1))]
        helper = calls_one.search(line)
        if helper and helper.group(1) in helpers:
            built = helpers[helper.group(1)]
        stored = re.search(rf"putstatic .*// Field (\w+):L{re.escape(type_class)};", line)
        if stored:
            # A type names itself in the registry, or is named after the field holding it.
            name = literal or stored.group(1).lower()
            name = name if ":" in name else f"minecraft:{name}"
            if built is not None:
                found[name] = jar.real(built)
            elif name in BUILDS_ITSELF:
                found[name] = BUILDS_ITSELF[name]
            built = None
            literal = None
    return found


def ancestry(jar: Jar, leaf: str) -> list[str]:
    """The chain from `Entity` down to a class, which is what fixes where its fields sit."""
    chain = []
    name = leaf
    while name and name != ENTITY:
        chain.append(name)
        listing = jar.listing(name)
        found = re.search(r"\bextends ([\w.$]+)", listing.split("{", 1)[0])
        if not found:
            break
        name = jar.real(found.group(1).replace(".", "/"))
    chain.append(ENTITY)
    chain.reverse()
    return chain


def declarations(jar: Jar, owner: str) -> list[tuple[str, str]]:
    """The fields a class declares, in order, as the serializer and the name of each."""
    listing = jar.listing(owner)
    if not listing:
        return []
    serializers = jar.path_of(SERIALIZERS)
    synched = jar.path_of(SYNCHED)
    define = jar.renames.obfuscated_method(SYNCHED, DEFINE)

    fields: list[tuple[str, str]] = []
    on: str | None = None
    kind: str | None = None
    for line in clinit(listing).splitlines():
        target = re.search(r"ldc\w*\s+#\d+\s+// class ([\w/$]+)", line)
        if target:
            on = jar.real(target.group(1))
        held = re.search(rf"getstatic .*// Field {re.escape(serializers)}\.(\w+):", line)
        if held:
            kind = jar.renames.real_field(SERIALIZERS, held.group(1)).lower()
        called = re.search(
            rf"invokestatic .*// Method {re.escape(synched)}\.{re.escape(define)}:", line
        )
        if called:
            if kind is None:
                raise SystemExit(f"{owner}: a field was defined with no kind in front of it")
            if on is not None and on != owner:
                raise SystemExit(f"{owner} defines a field on {on}, which nothing here expects")
            fields.append((kind, None))
            kind = None
        stored = re.search(r"putstatic\s+#\d+\s+// Field (\w+):", line)
        if stored and fields and fields[-1][1] is None:
            fields[-1] = (fields[-1][0], jar.renames.real_field(owner, stored.group(1)))
    return [(kind, name or "?") for kind, name in fields]


def layouts(version: str, work: Path, cache: Path) -> dict:
    jar = Jar(version, work, cache)
    built_from = implementations(jar)
    jar.disassemble(list(built_from.values()))

    declared: dict[str, list[tuple[str, str]]] = {}
    out: dict[str, dict] = {}
    for name, leaf in sorted(built_from.items()):
        chain = ancestry(jar, leaf)
        for owner in chain:
            if owner not in declared:
                declared[owner] = declarations(jar, owner)
        fields = []
        for owner in chain:
            for kind, field in declared[owner]:
                fields.append(
                    {
                        "index": len(fields),
                        "serializer": kind,
                        # A nested class is named for itself, not for the one holding it.
                        "owner": f"{owner.rsplit('.', 1)[-1].rsplit('$', 1)[-1]}#{field}",
                        "name": field,
                    }
                )
        out[name] = {"class": leaf, "fields": fields}
    return {"version": version, "types": out}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", nargs="?")
    parser.add_argument("--all", action="store_true")
    parser.add_argument("--cache", type=Path, default=DEFAULT_CACHE)
    args = parser.parse_args()

    import tempfile

    for version in VERSIONS if args.all else [args.version or VERSIONS[-1]]:
        with tempfile.TemporaryDirectory() as name:
            data = layouts(version, Path(name), args.cache)
        out = REPO_ROOT / "assets" / "extracted" / version / "field_layouts.json"
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(data, indent=1) + "\n")
        print(f"{version}: {len(data['types'])} types -> {out.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()
