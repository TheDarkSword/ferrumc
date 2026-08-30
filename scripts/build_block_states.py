#!/usr/bin/env python3
"""Generate src/lib/world/src/block_state/generated.rs from the vanilla block report.

A block state id is an index into one block's cartesian product of property values, and the blocks
partition the id space in order with no gaps. That makes every question about a state arithmetic
rather than a lookup: which block it is, what a property of it reads, and what id the same block
with one property changed has.

The order the properties are combined in is *not* the order the report lists them, and for twelve
blocks the two disagree, so each property's stride is measured from the states themselves rather
than assumed.

Usage:
    scripts/build_block_states.py
"""

from __future__ import annotations

import json
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
REPORT = REPO_ROOT / "assets" / "extracted" / "26.2" / "reports" / "blocks.json"
OUT = REPO_ROOT / "src" / "lib" / "world" / "src" / "block_state" / "generated.rs"
# The name and properties behind every id, which the `block!` macro and the world importer resolve
# names through. Kept beside the generated tables so the two cannot disagree about a version.
STATES_JSON = REPO_ROOT / "assets" / "data" / "blockstates.json"


# What each enum-valued property actually is, taken from the vanilla sources: the declarations in
# `BlockStateProperties.java` and the enums they name. A wire name alone does not say - `half` is a
# stair's top or bottom and a door's upper or lower, `type` is a slab's, a chest's or a piston's -
# so the value set decides between the candidates.
#
# Values are listed in the order vanilla declares them. Anything here rather than derived is here
# because the sources are a decompilation that is not checked in.
TYPES: dict[str, list[str]] = {
    "Axis": ["x", "y", "z"],
    "Direction": ["down", "up", "north", "south", "west", "east"],
    "FrontAndTop": [
        "down_east", "down_north", "down_south", "down_west", "up_east", "up_north", "up_south",
        "up_west", "west_up", "east_up", "north_up", "south_up",
    ],
    "AttachFace": ["floor", "wall", "ceiling"],
    "BellAttachType": ["floor", "ceiling", "single_wall", "double_wall"],
    "WallSide": ["none", "low", "tall"],
    "RedstoneSide": ["up", "side", "none"],
    "DoubleBlockHalf": ["upper", "lower"],
    "Half": ["top", "bottom"],
    "SideChainPart": ["unconnected", "left", "center", "right"],
    "RailShape": [
        "north_south", "east_west", "ascending_east", "ascending_west", "ascending_north",
        "ascending_south", "south_east", "south_west", "north_west", "north_east",
    ],
    "BedPart": ["head", "foot"],
    "ChestType": ["single", "left", "right"],
    "ComparatorMode": ["compare", "subtract"],
    "DoorHingeSide": ["left", "right"],
    "NoteBlockInstrument": [
        "harp", "basedrum", "snare", "hat", "bass", "flute", "bell", "guitar", "chime",
        "xylophone", "iron_xylophone", "cow_bell", "didgeridoo", "bit", "banjo", "pling", "zombie",
        "skeleton", "creeper", "dragon", "wither_skeleton", "piglin", "custom_head", "trumpet",
        "trumpet_exposed", "trumpet_weathered", "trumpet_oxidized",
    ],
    "PistonType": ["normal", "sticky"],
    "SlabType": ["top", "bottom", "double"],
    "StairsShape": ["straight", "inner_left", "inner_right", "outer_left", "outer_right"],
    "StructureMode": ["save", "load", "corner", "data"],
    "BambooLeaves": ["none", "small", "large"],
    "Tilt": ["none", "unstable", "partial", "full"],
    "SpeleothemThickness": ["tip_merge", "tip", "frustum", "middle", "base"],
    "SculkSensorPhase": ["inactive", "active", "cooldown"],
    "TrialSpawnerState": [
        "inactive", "waiting_for_players", "active", "waiting_for_reward_ejection",
        "ejecting_reward", "cooldown",
    ],
    "VaultState": ["inactive", "active", "unlocking", "ejecting"],
    "CreakingHeartState": ["uprooted", "dormant", "awake"],
    "TestBlockMode": ["start", "log", "fail", "accept"],
    "CopperGolemPose": ["standing", "sitting", "star", "running"],
    "PotentSulfurState": ["dormant", "dry", "wet", "continuous", "erupting"],
}

# The types a wire name can carry, so a value set is only weighed against the ones it could be.
WIRE_TYPES: dict[str, list[str]] = {
    "axis": ["Axis"],
    "facing": ["Direction"],
    "vertical_direction": ["Direction"],
    "orientation": ["FrontAndTop"],
    "face": ["AttachFace"],
    "attachment": ["BellAttachType"],
    "east": ["WallSide", "RedstoneSide"],
    "north": ["WallSide", "RedstoneSide"],
    "south": ["WallSide", "RedstoneSide"],
    "west": ["WallSide", "RedstoneSide"],
    "half": ["DoubleBlockHalf", "Half"],
    "side_chain": ["SideChainPart"],
    "shape": ["RailShape", "StairsShape"],
    "part": ["BedPart"],
    "type": ["ChestType", "PistonType", "SlabType"],
    "mode": ["ComparatorMode", "StructureMode", "TestBlockMode"],
    "hinge": ["DoorHingeSide"],
    "instrument": ["NoteBlockInstrument"],
    "leaves": ["BambooLeaves"],
    "tilt": ["Tilt"],
    "thickness": ["SpeleothemThickness"],
    "sculk_sensor_phase": ["SculkSensorPhase"],
    "trial_spawner_state": ["TrialSpawnerState"],
    "vault_state": ["VaultState"],
    "creaking_heart_state": ["CreakingHeartState"],
    "copper_golem_pose": ["CopperGolemPose"],
    "potent_sulfur_state": ["PotentSulfurState"],
}


def value_type(name: str, values: tuple[str, ...]) -> str:
    """What a property's values are, as a Rust type."""
    unique = set(values)
    if unique == {"true", "false"}:
        return "bool"
    if all(value.isdigit() for value in unique):
        return "u8"
    candidates = [
        kind for kind in WIRE_TYPES.get(name, []) if unique <= set(TYPES[kind])
    ]
    if len(candidates) != 1:
        raise SystemExit(
            f"`{name}` taking {sorted(unique)} matches {candidates or 'no type'}; "
            "the tables in this script need the new one adding"
        )
    return candidates[0]


def variant(name: str) -> str:
    """A property name as a Rust enum variant."""
    return "".join(part.capitalize() for part in name.split("_"))


def strides(block: dict) -> dict[str, int]:
    """How far apart two states differing only in one property are.

    Measured rather than derived: the report's property order is not always the order the ids were
    built in.
    """
    properties = block["properties"]
    by_values = {
        tuple(state["properties"][name] for name in properties): state["id"]
        for state in block["states"]
    }
    reference = next(iter(by_values))
    out = {}
    for position, name in enumerate(properties):
        values = properties[name]
        first = list(reference)
        first[position] = values[0]
        second = list(reference)
        second[position] = values[1]
        out[name] = by_values[tuple(second)] - by_values[tuple(first)]
    return out


def main() -> None:
    with REPORT.open() as handle:
        report = json.load(handle)

    # Distinct (property, values) pairs, since the same name carries different values on different
    # blocks: a stair faces four ways and a piston six.
    value_sets: dict[tuple[str, tuple[str, ...]], int] = {}
    names: dict[str, None] = {}
    for block in report.values():
        for name, values in block.get("properties", {}).items():
            names[name] = None
            value_sets.setdefault((name, tuple(values)), len(value_sets))

    blocks = []
    for name, block in report.items():
        states = block["states"]
        base = min(state["id"] for state in states)
        default = next(state["id"] for state in states if state.get("default"))
        properties = block.get("properties", {})
        measured = strides(block) if properties else {}
        entries = [
            (value_sets[(prop, tuple(values))], measured[prop])
            for prop, values in properties.items()
        ]

        # Everything below rests on this holding, so it is checked rather than trusted.
        for state in states:
            expected = base + sum(
                properties[prop].index(state["properties"][prop]) * measured[prop]
                for prop in properties
            )
            if expected != state["id"]:
                raise SystemExit(f"{name} state {state['id']} is not where its properties put it")

        blocks.append((base, name, default, len(states), entries))

    blocks.sort()

    lines = [
        "//! Block states, their blocks and their properties.",
        "//!",
        "//! Generated by `scripts/build_block_states.py` from the vanilla block report. Do not edit.",
        "",
        "/// Every property name any block carries.",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]",
        "pub enum Property {",
    ]
    lines += [f"    {variant(name)}," for name in names]
    lines += [
        "}",
        "",
        "impl Property {",
        "    /// The name this property has in block state ids and commands.",
        "    #[must_use]",
        "    pub const fn name(self) -> &'static str {",
        "        match self {",
    ]
    lines += [f'            Self::{variant(name)} => "{name}",' for name in names]
    lines += [
        "        }",
        "    }",
        "",
        "    /// The property of this name, if any block carries one.",
        "    #[must_use]",
        "    pub fn from_name(name: &str) -> Option<Self> {",
        "        match name {",
    ]
    lines += [f'            "{name}" => Some(Self::{variant(name)}),' for name in names]
    lines += [
        "            _ => None,",
        "        }",
        "    }",
        "}",
        "",
        "/// One property as a particular block offers it, since the same name can take different",
        "/// values on different blocks.",
        "pub struct PropertyValues {",
        "    pub property: Property,",
        "    pub values: &'static [&'static str],",
        "}",
        "",
        f"pub static PROPERTY_VALUES: [PropertyValues; {len(value_sets)}] = [",
    ]
    for (name, values), _ in sorted(value_sets.items(), key=lambda pair: pair[1]):
        rendered = ", ".join(f'"{value}"' for value in values)
        lines.append(
            f"    PropertyValues {{ property: Property::{variant(name)}, values: &[{rendered}] }},"
        )
    lines += [
        "];",
        "",
        "/// A block, and where its states sit in the id space.",
        "pub struct BlockDef {",
        "    pub name: &'static str,",
        "    pub base_state: u32,",
        "    pub default_state: u32,",
        "    pub state_count: u32,",
        "    /// Each property this block has, as an index into [`PROPERTY_VALUES`] and how far",
        "    /// apart two states differing only in it are.",
        "    pub properties: &'static [(u16, u32)],",
        "}",
        "",
        "/// Sorted by `base_state`, which partitions the id space with no gaps.",
        f"pub static BLOCKS: [BlockDef; {len(blocks)}] = [",
    ]
    for base, name, default, count, entries in blocks:
        rendered = ", ".join(f"({index}, {stride})" for index, stride in entries)
        lines.append(
            f'    BlockDef {{ name: "{name}", base_state: {base}, default_state: {default}, '
            f"state_count: {count}, properties: &[{rendered}] }},"
        )
    lines += ["];", ""]

    # Which Rust type each property's values are, so a caller names a variant rather than a string.
    typed = {}
    for (name, values) in value_sets:
        typed.setdefault(name, set()).add(value_type(name, values))

    used = sorted({kind for kinds in typed.values() for kind in kinds} - {"bool", "u8"})
    for kind in used:
        lines += [
            f"/// The values `{kind}` properties take.",
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]",
            f"pub enum {kind} {{",
        ]
        lines += [f"    {variant(value)}," for value in TYPES[kind]]
        lines += [
            "}",
            "",
            f"impl crate::block_state::PropertyValue for {kind} {{",
            "    fn name(self) -> &'static str {",
            "        match self {",
        ]
        lines += [
            f'            Self::{variant(value)} => "{value}",' for value in TYPES[kind]
        ]
        lines += [
            "        }",
            "    }",
            "",
            "    fn from_name(name: &str) -> Option<Self> {",
            "        match name {",
        ]
        lines += [
            f'            "{value}" => Some(Self::{variant(value)}),' for value in TYPES[kind]
        ]
        lines += ["            _ => None,", "        }", "    }", "}", ""]

    # One constant per property and type it can carry. A block has at most one property of a given
    # name, so asking for the wrong type of it simply finds nothing.
    constants = {}
    for name, kinds in typed.items():
        for kind in kinds:
            if len(kinds) == 1:
                constants[(name, kind)] = name.upper()
            else:
                snake = "".join(
                    f"_{ch.lower()}" if ch.isupper() and index else ch.lower()
                    for index, ch in enumerate(kind)
                )
                constants[(name, kind)] = snake.upper()
    taken = {}
    for key, label in constants.items():
        taken.setdefault(label, []).append(key)
    for label, keys in taken.items():
        if len(keys) > 1:
            for name, kind in keys:
                constants[(name, kind)] = f"{name.upper()}_{label}"

    lines += [
        "/// Every property, named so that reading or setting it names its type as well.",
        "pub mod properties {",
        "    use super::*;",
        "    use crate::block_state::BlockProperty;",
        "",
    ]
    for (name, kind), label in sorted(constants.items(), key=lambda pair: pair[1]):
        lines.append(
            f"    pub const {label}: BlockProperty<{kind}> = "
            f"BlockProperty::new(Property::{variant(name)});"
        )
    lines += ["}", ""]

    states_json = {}
    for name, block in report.items():
        for state in block["states"]:
            entry = {"name": name}
            if state.get("properties"):
                entry["properties"] = state["properties"]
            if state.get("default"):
                entry["default"] = True
            states_json[str(state["id"])] = entry
    STATES_JSON.write_text(json.dumps(states_json, separators=(",", ":"), sort_keys=True))
    print(f"{len(states_json)} states -> {STATES_JSON.relative_to(REPO_ROOT)}")

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text("\n".join(lines))
    print(
        f"{len(blocks)} blocks, {len(names)} property names, {len(value_sets)} value sets "
        f"-> {OUT.relative_to(REPO_ROOT)}"
    )


if __name__ == "__main__":
    main()
