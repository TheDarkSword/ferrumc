#!/usr/bin/env python3
"""Generate the block shape tables from assets/extracted/<version>/block_shapes.json.

A block state's collision shape is a handful of boxes - 1.85 of them on average, fifteen at the
worst, and none at all for a third of the states - so they are kept as a plain list rather than the
bitmap over per-axis coordinates vanilla uses. At that size the bitmap is all overhead.

Boxes and shapes are shared: 32366 states resolve to 683 distinct boxes and 915 distinct shapes, so
the tables are small enough to sit in the binary as ordinary constants. Only the two arrays mapping
a state to its shape are large, and those go in as bytes.

Usage:
    scripts/build_block_shapes.py
"""

from __future__ import annotations

import json
import struct
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
NATIVE = "26.2"
SOURCE = REPO_ROOT / "assets" / "extracted" / NATIVE / "block_shapes.json"
OUT_RS = REPO_ROOT / "src" / "lib" / "world" / "src" / "block_shape" / "generated.rs"
OUT_BIN = REPO_ROOT / "assets" / "data" / "block_shapes"


def number(value: float) -> str:
    """A coordinate as a Rust literal, keeping the sixteenths exact."""
    return f"{float(value)!r}"


def main() -> None:
    with SOURCE.open() as handle:
        data = json.load(handle)

    if data["version"] != NATIVE:
        raise SystemExit(f"{SOURCE} holds {data['version']}, not {NATIVE}")

    boxes = data["boxes"]
    shapes = data["shapes"]
    states = data["states"]

    OUT_BIN.mkdir(parents=True, exist_ok=True)
    for field in ("collision", "outline"):
        indices = [0 if state is None else state[field] for state in states]
        (OUT_BIN / f"{field}.bin").write_bytes(struct.pack(f"<{len(indices)}H", *indices))

    # Which block entity each block carries, as its registry id, or none. Keyed on block rather
    # than state: every state of a chest is a chest.
    entities = data["block_entities"]
    order = {entry["block"]: entry["type"] for entry in entities}
    blocks_json = json.loads(
        (REPO_ROOT / "assets" / "extracted" / NATIVE / "reports" / "blocks.json").read_text()
    )
    by_base = sorted(
        blocks_json, key=lambda name: min(st["id"] for st in blocks_json[name]["states"])
    )
    owners = [order.get(name, -1) for name in by_base]
    (OUT_BIN / "block_entities.bin").write_bytes(
        struct.pack(f"<{len(owners)}H", *[0xFFFF if o < 0 else o for o in owners])
    )

    # Which face shape each of a state's six sides has, and whether light is stopped between any
    # pair of them. Whether light passes between two blocks is a question about both their faces
    # together; there are few enough distinct faces that every pair's answer fits in a table.
    faces = bytearray(len(states) * 6)
    for index, state in enumerate(states):
        if state is None:
            continue
        for side, shape in enumerate(state["face_shapes"]):
            faces[index * 6 + side] = shape
    (OUT_BIN / "face_shapes.bin").write_bytes(faces)

    pairs = data["face_occlusion_pairs"]
    stride = (len(pairs) + 7) // 8
    matrix = bytearray(len(pairs) * stride)
    for row, answers in enumerate(pairs):
        for column, occludes in enumerate(answers):
            if occludes:
                matrix[row * stride + column // 8] |= 1 << (column % 8)
    (OUT_BIN / "face_occlusion.bin").write_bytes(matrix)

    # How each state deals with light: what it emits, how much it dims what passes through, and
    # the two flags the engines branch on. Two bytes a state.
    light = bytearray(len(states) * 2)
    for index, state in enumerate(states):
        if state is None:
            continue
        light[index * 2] = state["light_emission"] | (state["light_dampening"] << 4)
        # One byte: two flags and one bit per face saying whether that face stops light by itself.
        # Kept for the cheap answer; the pair table above settles the rest.
        flags = state["face_occludes_light"] << 2
        if state["shape_occludes_light"]:
            flags |= 1
        if state["propagates_skylight"]:
            flags |= 2
        light[index * 2 + 1] = flags
    (OUT_BIN / "light.bin").write_bytes(light)

    # Which faces of each state hold something up: one bit per direction and support type, in the
    # game's own order, so three bytes a state.
    sturdy = bytearray(len(states) * 3)
    for index, state in enumerate(states):
        bits = 0 if state is None else state["face_sturdy"]
        sturdy[index * 3 : index * 3 + 3] = bits.to_bytes(3, "little")
    (OUT_BIN / "face_sturdy.bin").write_bytes(sturdy)

    # Which states take a random tick, as one bit each. The random tick loop asks this of thousands
    # of positions a second, and a section that holds none of them is skipped entirely.
    ticking = bytearray((len(states) + 7) // 8)
    for index, state in enumerate(states):
        if state is not None and state["randomly_ticking"]:
            ticking[index // 8] |= 1 << (index % 8)
    (OUT_BIN / "randomly_ticking.bin").write_bytes(ticking)

    # One bit per state: whether a mob may stand on it. The spawn loop asks this of every position
    # it tries, which is thousands a second, so it is one bit test.
    spawnable = bytearray((len(states) + 7) // 8)
    for index, state in enumerate(states):
        if state is not None and state["valid_spawn"]:
            spawnable[index // 8] |= 1 << (index % 8)
    (OUT_BIN / "valid_spawn.bin").write_bytes(spawnable)

    lines = [
        "//! Which boxes each block state occupies.",
        "//!",
        "//! Generated by `scripts/build_block_shapes.py` from the extracted shapes. Do not edit.",
        "",
        "use super::Aabb;",
        "",
        "/// Every distinct box any shape is built from, in block-relative coordinates.",
        f"pub static BOXES: [Aabb; {len(boxes)}] = [",
    ]
    for min_x, min_y, min_z, max_x, max_y, max_z in boxes:
        lines.append(
            "    Aabb::new("
            f"{number(min_x)}, {number(min_y)}, {number(min_z)}, "
            f"{number(max_x)}, {number(max_y)}, {number(max_z)}),"
        )
    lines += [
        "];",
        "",
        "/// Every distinct shape, as indices into [`BOXES`]. An empty one is a block nothing",
        "/// collides with.",
        f"pub static SHAPES: [&[u16]; {len(shapes)}] = [",
    ]
    for shape in shapes:
        lines.append("    &[" + ", ".join(str(index) for index in shape) + "],")
    lines += ["];", ""]

    OUT_RS.parent.mkdir(parents=True, exist_ok=True)
    OUT_RS.write_text("\n".join(lines))
    randomly_ticking = sum(
        1 for state in states if state is not None and state["randomly_ticking"]
    )
    print(f"{randomly_ticking} states take a random tick")
    print(f"{sum(1 for o in owners if o >= 0)} blocks carry a block entity")
    print(f"{len(pairs)} distinct face shapes, {len(pairs) * len(pairs)} pair answers")
    print(
        f"{len(states)} states, {len(shapes)} shapes, {len(boxes)} boxes "
        f"-> {OUT_RS.relative_to(REPO_ROOT)} and {OUT_BIN.relative_to(REPO_ROOT)}/"
    )


if __name__ == "__main__":
    main()
