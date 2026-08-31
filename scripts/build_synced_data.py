#!/usr/bin/env python3
"""Generate src/lib/entities/src/synced_data/generated.rs from the extracted synced data.

Three things are written down here, and none of them can be guessed at:

- the serializer numbers, which say what kind of value follows a field on the wire and are handed
  out in registration order;
- what fields each entity type carries and in what order, which vanilla works out by walking the
  entity class tree;
- the vocabularies a field can hold, so nothing downstream has to name a bare number.

Run scripts/extract_synched_data.py first, for every version listed in VERSIONS.

Usage:
    scripts/build_synced_data.py
"""

from __future__ import annotations

import json
import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
EXTRACTED = REPO_ROOT / "assets" / "extracted"
OUT = REPO_ROOT / "src" / "lib" / "entities" / "src" / "synced_data" / "generated.rs"

# The version the server's own world model is built for.
NATIVE = "26.2"
# Every version whose layout is known. The older ones ship an obfuscated jar, so nothing can be
# read out of them; see the deferred note in internal_docs.
KNOWN = ["26.1", "26.2"]

# What a client reads instead of an index, for a field its version does not have.
ABSENT = 255

# Every version the server speaks, oldest first, matching `ProtocolVersion::ALL`. Each one is asked
# what number every kind of value travels as, which is the one thing about a row that can be read
# out of a jar that carries no names.
SPOKEN = [
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

# A kind of value the game renamed without changing what it writes.
RENAMED = {"optional_uuid": "optional_living_entity_reference"}

# How each kind of value is held. A kind with no entry here is one nothing sets yet: its default
# travels as the bytes the game wrote for it, and it gets a shape of its own when something needs
# to write one.
HELD_AS = {
    "byte": ("Byte", "u8"),
    "int": ("Int", "i32"),
    "long": ("Long", "i64"),
    "float": ("Float", "f32"),
    "string": ("Text", "String"),
    "component": ("Component", "TextComponent"),
    "optional_component": ("OptionalComponent", "Option<TextComponent>"),
    "item_stack": ("Item", "InventorySlot"),
    "boolean": ("Boolean", "bool"),
    "rotations": ("Rotations", "[f32; 3]"),
    "block_pos": ("BlockPos", "NetworkPosition"),
    "optional_block_pos": ("OptionalBlockPos", "Option<NetworkPosition>"),
    "direction": ("Direction", "Direction"),
    "pose": ("Pose", "Pose"),
    "sniffer_state": ("SnifferState", "SnifferState"),
    "armadillo_state": ("ArmadilloState", "ArmadilloState"),
    "copper_golem_state": ("CopperGolemState", "CopperGolemState"),
    "weathering_copper_state": ("WeatheringState", "WeatheringState"),
    "vector3": ("Vector3", "[f32; 3]"),
    "quaternion": ("Quaternion", "[f32; 4]"),
    "humanoid_arm": ("Arm", "Arm"),
}

# The vocabularies, and the name each becomes in Rust.
VOCABULARIES = {
    "pose": "Pose",
    "direction": "Direction",
    "humanoid_arm": "Arm",
    "sniffer_state": "SnifferState",
    "armadillo_state": "ArmadilloState",
    "copper_golem_state": "CopperGolemState",
    "weathering_copper_state": "WeatheringState",
}


def camel(name: str) -> str:
    """`fall_flying` -> `FallFlying`."""
    return "".join(part.capitalize() for part in name.split("_"))


def snake(name: str) -> str:
    """`AbstractCubeMob` -> `abstract_cube_mob`."""
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def variant(name: str) -> str:
    """`minecraft:acacia_boat` -> `AcaciaBoat`."""
    return camel(name.removeprefix("minecraft:"))


def constant(field: str) -> str:
    """`DATA_SHARED_FLAGS_ID` -> `SHARED_FLAGS`.

    Vanilla decorates its accessor names with a prefix, a suffix or both, and none of it says
    anything the type does not already say.
    """
    name = field.removeprefix("DATA_").removeprefix("ID_")
    name = name.removesuffix("_ID")
    return name or field


class Reader:
    """Reads back a default value from the bytes the game wrote for it."""

    def __init__(self, data: bytes) -> None:
        self.data = data
        self.at = 0

    def take(self, count: int) -> bytes:
        chunk = self.data[self.at : self.at + count]
        if len(chunk) != count:
            raise ValueError("ran out of bytes")
        self.at += count
        return chunk

    def var_int(self) -> int:
        value = 0
        for shift in range(0, 35, 7):
            byte = self.take(1)[0]
            value |= (byte & 0x7F) << shift
            if not byte & 0x80:
                # Sign-extend, since the wire form is a two's complement i32.
                return value - (1 << 32) if value & (1 << 31) else value
        raise ValueError("varint too long")

    def var_long(self) -> int:
        value = 0
        for shift in range(0, 70, 7):
            byte = self.take(1)[0]
            value |= (byte & 0x7F) << shift
            if not byte & 0x80:
                return value - (1 << 64) if value & (1 << 63) else value
        raise ValueError("varlong too long")

    def float(self) -> float:
        import struct

        return struct.unpack(">f", self.take(4))[0]

    def done(self) -> bool:
        return self.at == len(self.data)


def rust_float(value: float) -> str:
    text = repr(float(value))
    if text in ("inf", "-inf", "nan"):
        raise ValueError(f"cannot write {text} as a literal")
    return f"{text}f32"


def default_expr(serializer: str, raw: str, enums: dict[str, dict[str, int]]) -> str:
    """The default value as Rust, read back from what the game wrote."""
    data = bytes.fromhex(raw)
    held = HELD_AS.get(serializer)
    if held is None:
        return raw_expr(data)

    name, _ = held
    read = Reader(data)
    try:
        if serializer == "byte":
            value = f"DataValue::Byte({read.take(1)[0]})"
        elif serializer == "int":
            value = f"DataValue::Int({read.var_int()})"
        elif serializer == "long":
            value = f"DataValue::Long({read.var_long()})"
        elif serializer == "float":
            value = f"DataValue::Float({rust_float(read.float())})"
        elif serializer == "boolean":
            value = f"DataValue::Boolean({str(read.take(1)[0] != 0).lower()})"
        elif serializer == "string":
            if read.var_int() != 0:
                # Only an empty string can be written as a constant, and every default is one.
                return raw_expr(data)
            value = "DataValue::Text(String::new())"
        elif serializer in ("rotations", "vector3"):
            axes = ", ".join(rust_float(read.float()) for _ in range(3))
            value = f"DataValue::{name}([{axes}])"
        elif serializer == "quaternion":
            axes = ", ".join(rust_float(read.float()) for _ in range(4))
            value = f"DataValue::Quaternion([{axes}])"
        elif serializer == "block_pos":
            packed = int.from_bytes(read.take(8), "big", signed=True)
            x, y, z = packed >> 38, (packed << 52) >> 52, (packed << 26) >> 38
            value = f"DataValue::BlockPos(NetworkPosition {{ x: {x}, y: {y}, z: {z} }})"
        elif serializer == "optional_block_pos":
            if read.take(1)[0] != 0:
                return raw_expr(data)
            value = "DataValue::OptionalBlockPos(None)"
        elif serializer == "optional_component":
            if read.take(1)[0] != 0:
                return raw_expr(data)
            value = "DataValue::OptionalComponent(None)"
        elif serializer in VOCABULARIES:
            by_id = {v: k for k, v in enums[serializer].items()}
            value = f"DataValue::{name}({VOCABULARIES[serializer]}::{camel(by_id[read.var_int()])})"
        else:
            return raw_expr(data)
    except (ValueError, KeyError):
        return raw_expr(data)

    return value if read.done() else raw_expr(data)


def raw_expr(data: bytes) -> str:
    body = ", ".join(f"0x{byte:02x}" for byte in data)
    return f"DataValue::Raw(&[{body}])"


def load(version: str) -> dict:
    path = EXTRACTED / version / "synched_data.json"
    if not path.exists():
        raise SystemExit(f"{path} is missing; run scripts/extract_synched_data.py {version}")
    return json.loads(path.read_text())


def main() -> None:
    native = load(NATIVE)
    others = {version: load(version) for version in KNOWN if version != NATIVE}

    serializers = native["serializers"]
    for version, data in others.items():
        if data["serializers"] != serializers:
            raise SystemExit(
                f"{version} numbers the serializers differently; the table needs a version arm"
            )

    enums = native["enums"]
    types = native["types"]

    for name, entry in types.items():
        named = [constant(field["owner"].split("#")[1]) for field in entry["fields"]]
        if len(set(named)) != len(named):
            raise SystemExit(f"{name} names two fields the same, so nothing can match them across versions")
        if len(entry["fields"]) > 64:
            raise SystemExit(f"{name} carries {len(entry['fields'])} fields; the change word has 64")

    # A field's index and kind depend only on the class that declared it, which is what makes one
    # constant per field correct for every type that inherits it.
    owners: dict[str, tuple[int, str]] = {}
    for name, entry in types.items():
        for field in entry["fields"]:
            seen = owners.setdefault(field["owner"], (field["index"], field["serializer"]))
            if seen != (field["index"], field["serializer"]):
                raise SystemExit(f"{field['owner']} sits at two places: {seen} and {field}")

    lines: list[str] = []
    add = lines.append

    add("//! What a client is told about an entity, and where each part of it sits.")
    add("//!")
    add(f"//! Generated by `scripts/build_synced_data.py` from the {NATIVE} data. Do not edit.")
    add("//!")
    add("//! Vanilla hands out a field's index by walking the entity class tree, so a field's place")
    add("//! depends on every class above it, and its kind travels as the number its serializer was")
    add("//! registered under. Both move between versions and neither is written down anywhere, so")
    add("//! both are read out of the game rather than transcribed.")
    add("")
    add("#![allow(clippy::unreadable_literal)]")
    add("")
    add("use super::value::{DataValue, Field};")
    add("use crate::entity_type::EntityType;")
    add("use ferrumc_inventories::slot::InventorySlot;")
    add("use ferrumc_net_codec::net_types::network_position::NetworkPosition;")
    add("use ferrumc_net_codec::version::ProtocolVersion;")
    add("use ferrumc_text::TextComponent;")
    add("")

    # --- the serializer numbers -------------------------------------------------------------
    add("/// What kind of value a field holds, numbered as the game numbers it on the wire.")
    add("///")
    add("/// The numbers are registration order rather than anything declared, so the variants")
    add("/// carry them as their own discriminants and the tag written down a connection is the")
    add("/// variant itself.")
    add("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]")
    add("#[repr(u8)]")
    add("pub enum Serializer {")
    for number, name in enumerate(serializers):
        add(f"    {camel(name)} = {number},")
    add("}")
    add("")
    # What each version numbers each kind, which is nothing like the same between them: 1.21 has
    # no place at all for two thirds of what 26.2 can write, and puts a pose one along from where
    # 26.2 puts it.
    spoken: dict[str, list[str]] = {}
    for version in SPOKEN:
        path = EXTRACTED / version / "serializer_ids.json"
        if not path.exists():
            raise SystemExit(f"{path} is missing; run scripts/extract_serializer_ids.py --all")
        spoken[version] = [
            RENAMED.get(name, name) for name in json.loads(path.read_text())["serializers"]
        ]
    if spoken[NATIVE] != serializers:
        raise SystemExit(
            f"the two readings of {NATIVE} disagree; one of the extractors is wrong"
        )

    add("/// What number each kind travels as, for each version the server speaks.")
    add("///")
    add("/// Read out of every version's own jar rather than translated from the newest: the")
    add("/// numbers are registration order and the order moves, so a client sent the wrong one")
    add("/// does not lose a value, it reads the bytes as whatever kind it does keep there and")
    add("/// then reads the rest of the row at the wrong offset.")
    add(f"const WIRE_IDS: [[u8; {len(serializers)}]; {len(SPOKEN)}] = [")
    for version in SPOKEN:
        places = {name: number for number, name in enumerate(spoken[version])}
        row = ", ".join(str(places.get(name, ABSENT)) for name in serializers)
        add(f"    [{row}], // {version}")
    add("];")
    add("")
    add("impl Serializer {")
    add("    /// The number a client speaking `version` reads to know what kind of value follows,")
    add("    /// or nothing where that version has no such kind.")
    add("    #[must_use]")
    add("    pub const fn wire_id(self, version: ProtocolVersion) -> Option<u8> {")
    add("        match WIRE_IDS[version.index()][self as usize] {")
    add("            ABSENT => None,")
    add("            id => Some(id),")
    add("        }")
    add("    }")
    add("}")
    add("")

    # --- the vocabularies -------------------------------------------------------------------
    for key, rust_name in VOCABULARIES.items():
        values = enums[key]
        add(f"/// The values a `{key}` field can hold.")
        add("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]")
        add("#[repr(u8)]")
        add(f"pub enum {rust_name} {{")
        for name, number in sorted(values.items(), key=lambda pair: pair[1]):
            add(f"    {camel(name)} = {number},")
        add("}")
        add("")
        add(f"impl {rust_name} {{")
        add("    /// The number this value travels as.")
        add("    #[must_use]")
        add("    pub const fn wire_id(self) -> i32 {")
        add("        self as i32")
        add("    }")
        add("}")
        add("")

    # --- the layouts ------------------------------------------------------------------------
    add("/// One field of one entity type: what kind of value it holds, and what it holds until")
    add("/// something sets it.")
    add("pub struct Slot {")
    add("    pub serializer: Serializer,")
    add("    pub default: DataValue,")
    add("}")
    add("")
    add("/// What each entity type carries, in the order a client reads it.")
    add(f"pub(crate) static LAYOUTS: [&[Slot]; {len(types)}] = [")
    for name, entry in types.items():
        add(f"    // {name}")
        add("    &[")
        for field in entry["fields"]:
            default = default_expr(field["serializer"], field["default"], enums)
            add(
                f"        Slot {{ serializer: Serializer::{camel(field['serializer'])},"
                f" default: {default} }}, // {field['owner']}"
            )
        add("    ],")
    add("];")
    add("")

    # --- where a field sits for each client ----------------------------------------------
    # Read out of every version's own jar. The names are matched rather than the places: a class
    # can be split or renamed between versions while the field it declared stays the same field,
    # which is what a client is actually reading.
    def named(field: dict) -> tuple[str, str]:
        return (constant(field["owner"].split("#")[1]), field["serializer"])

    places_for: dict[str, dict[str, list[int]]] = {}
    for version in SPOKEN:
        path = EXTRACTED / version / "field_layouts.json"
        if not path.exists():
            raise SystemExit(f"{path} is missing; run scripts/extract_field_layouts.py --all")
        there = json.loads(path.read_text())["types"]
        rows: dict[str, list[int]] = {}
        for name, entry in types.items():
            older = there.get(name)
            if older is None:
                rows[name] = [ABSENT] * len(entry["fields"])
                continue
            at = {named(f): f["index"] for f in older["fields"]}
            mapped = [at.get(named(f), ABSENT) for f in entry["fields"]]
            # Nothing moved is by far the common case, and an empty row says so in no bytes.
            rows[name] = [] if mapped == list(range(len(mapped))) else mapped
        places_for[version] = rows

    if any(places_for[NATIVE].values()):
        raise SystemExit(
            f"the two readings of {NATIVE} lay out an entity differently; one extractor is wrong"
        )

    add("/// What a client reads instead of an index, for a field its version has no place for.")
    add(f"pub const ABSENT: u8 = {ABSENT};")
    add("")
    add("/// Where each of a type's fields sits for a client of each version.")
    add("///")
    add("/// The server holds one entity in the newest version's terms, so a field's place for an")
    add("/// older client is a translation. A row of nothing means that version puts every field")
    add("/// exactly where this one does; a row of [`ABSENT`] means it has no such entity at all.")
    add(f"static PLACES: [[&[u8]; {len(types)}]; {len(SPOKEN)}] = [")
    for version in SPOKEN:
        add(f"    // {version}")
        add("    [")
        for name in types:
            row = places_for[version][name]
            body = ", ".join(str(place) for place in row)
            add(f"        &[{body}], // {name}")
        add("    ],")
    add("];")
    add("")
    add("/// Where a field of `kind` sits for a client speaking `version`, or [`ABSENT`].")
    add("#[must_use]")
    add("pub fn place_of(kind: EntityType, index: u8, version: ProtocolVersion) -> u8 {")
    add("    let row = PLACES[version.index()][kind as usize];")
    add("    if row.is_empty() {")
    add("        return index;")
    add("    }")
    add("    row.get(index as usize).copied().unwrap_or(ABSENT)")
    add("}")
    add("")

    # --- the field constants ----------------------------------------------------------------
    add("/// Every field a client reads, named the way the game names it and grouped by the class")
    add("/// that declares it, since that is what fixes where the field sits.")
    add("pub mod fields {")
    by_class: dict[str, list[tuple[str, int, str]]] = {}
    for owner, (index, serializer) in owners.items():
        class_name, field_name = owner.split("#")
        by_class.setdefault(class_name, []).append((field_name, index, serializer))
    for class_name in sorted(by_class):
        module = snake(class_name)
        add("")
        add(f"    /// The fields `{class_name}` declares.")
        add(f"    pub mod {module} {{")
        body: list[str] = []
        names: set[str] = set()
        for field_name, index, serializer in sorted(by_class[class_name], key=lambda f: f[1]):
            name = constant(field_name)
            if name in names:
                raise SystemExit(f"{class_name} has two fields called {name}")
            names.add(name)
            held = HELD_AS.get(serializer)
            if held is None:
                body.append(
                    f"        // {name} at {index} is a {serializer}, which nothing writes yet."
                )
                continue
            _, rust_type = held
            body.append(f"        /// `{field_name}` — {serializer}.")
            body.append(f"        pub const {name}: Field<{rust_type}> = Field::at({index});")
        # A module of nothing but notes has nothing to import.
        if any(line.lstrip().startswith("pub const") for line in body):
            add("        use super::super::*;")
        lines.extend(body)
        add("    }")
    add("}")
    add("")

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text("\n".join(lines))
    fields = sum(len(entry["fields"]) for entry in types.values())
    print(
        f"{len(serializers)} serializers, {len(types)} layouts, {fields} fields,"
        f" {len(owners)} named -> {OUT.relative_to(REPO_ROOT)}"
    )


if __name__ == "__main__":
    main()
