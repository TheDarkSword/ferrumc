# Known gaps in multi-version support

Seven of the ten supported versions work end to end: **26.2, 26.1, 1.21.11, 1.21.10, 1.21.8,
1.21.6 and 1.21.5** all join and play with no translation errors. Verify with
`scripts/check_versions.sh`.

Three do not, and neither failure is waiting on a feature that has yet to be built.

## 1.21.4 and 1.21.3 — the block palette after remapping

```
ERROR IN Protocol1_21_11To26_1 IN REMAP OF LEVEL_CHUNK_WITH_LIGHT
Caused by: java.lang.IndexOutOfBoundsException: Index (0) is greater than or equal to list size (0)
  at DataPaletteImpl.idAt
```

A section reports blocks but its palette reads as empty.

**Ruled out: the palette used to keep duplicate entries after translation.** The block state space
shrinks going backwards — 1335 targets in 1.21.4 receive more than one 26.2 state, and `stone` alone
receives 1120 — so a palette holding several states that all become stone ended up with duplicates,
which a reader keying on the id would have shortened. `PalettedContainer::from_paletted` now
deduplicates and rewrites the indices, collapsing to a single-valued section where everything
merges. It did not fix this failure.

**Also ruled out: terrain luck.** The seed used to be redrawn every launch, so two runs saw
different chunks and a version could pass or fail by accident. The seed is now configurable and
recorded with the world, and `check_versions.sh` pins it. The failure is reproducible and genuinely
version-dependent.

**Still unexplained.** A section's palette is translated entry by entry, and the block state space
shrinks going backwards — 32366 states in 26.2 become 27855 distinct ones in 1.21.4, with 1335
targets receiving more than one source state and `stone` alone receiving 1120. A palette holding
several states that all become stone therefore ends up with duplicate entries. A reader that keys
the palette by id sees fewer entries than the packed data still indexes, and the first lookup runs
off the end.

The fix is to rebuild the palette after remapping: deduplicate the entries and rewrite the data
array to the new indices, converting to a single-valued section when everything collapses to one
block. That work belongs in `PalettedContainer::from_paletted`.

Two other width bugs were found while looking for this, both fixed and both worth having
regardless. They did not resolve it.

- Direct block sections declared sixteen bits per entry. That width is `ceil(log2(block state
  count))` — fifteen for every supported version — and a strict reader sizes its reads by it.
- Direct biome palettes declared eight bits regardless of version, and packed by reinterpreting the
  backing memory rather than by writing at that width, which also assumed a host endianness.

Both were wrong for 26.2 as well. A lenient client accepts them, which is why they survived until a
translating proxy read the same bytes.

## 1.21 — the play login

```
ERROR IN Protocol1_21To1_21_2 IN REMAP OF LOGIN
```

The play `login` packet changed at 1.21.2 and no translator has been written for that boundary yet.
It is the largest remaining one — eighteen of the packets this server sends change there, against
one to five at every other boundary — and is ordinary outstanding work rather than a defect.

## Why none of this is blocked

Both failures are in code that exists. The chunk one is a defect in the block state remapping
written for this; the 1.21 one is a translator not yet written. Neither depends on block entities,
entity behaviour, world generation or anything else still to come.

The chunk defect is the more interesting of the two, because it is a property of the remapping
rather than of any one version: any target version whose block state space is smaller can collapse
a palette the same way.
