# Enchantments

What an enchantment does is written in the packs, not here. `ferrumc-data` reads the tree at build
time into something that can be asked a question.

## The shape

An effect is a **value that depends on the level**, hung under a **hook**, sometimes behind a
**requirement**.

```rust
Effect {
    hook: Hook::Damage,
    value: &LevelValue::Linear { base: 1.0, per_level: 0.5 },   // sharpness
    requires: Requires::Always,
}
```

Levels count from **one**, so `per_level_above_first` is added `level - 1` times: sharpness I is 1.0
and sharpness V is 3.0.

`LevelValue` also carries `LevelsSquared` — which is why efficiency runs away, level five being
`5² + 1 = 26` — plus fractions and clamps for the ones that need them.

## Four hooks are read

| hook | what it does | who |
|---|---|---|
| `Damage` | adds to a blow | sharpness, smite, bane of arthropods, impaling, power |
| `Protection` | takes off a blow | protection, feather falling, blast/fire/projectile protection |
| `Knockback` | adds to a push | knockback, punch |
| `Attribute` | moves one of the wearer's numbers | efficiency, respiration, aqua affinity, depth strider, soul speed, sweeping edge |

The `Attribute` hook is the interesting one: an enchantment is **not a special case**. Efficiency is
an `add_value` on `mining_efficiency`, aqua affinity an `add_multiplied_total` on
`submerged_mining_speed`. They go through the same modifier machinery armour does, named after the
slot, and come off exactly when the item does.

The other twenty-three kinds need hooks that do not exist yet and are **left out rather than
guessed at**.

## Requirements

Feather falling is protection that asks whether the blow was a fall. Getting that wrong would make
it armour against everything, so requirements are read where the shape is known and refused where it
is not:

```rust
Requires::DamageTags(&[("is_fall", true), ("bypasses_invulnerability", false)])
Requires::SomethingUnread    // never applies
```

Refusing is the cautious way round. An enchantment that guards against nothing is a gap; one that
guards against everything is a bug nobody notices until a player cannot be killed.

## The protection formula is not the armour formula

```rust
damage * (1.0 - protection.clamp(0.0, 20.0) / 25.0)
```

Flatter than armour's, and **nothing cuts through it**: twenty points of protection stop four fifths
of a one-point hit and four fifths of a hundred-point one. It is applied after resistance, and
`bypasses_enchantments` goes around it entirely.

It is worked out **per blow**, not kept on the wearer, because feather falling's answer depends on
what is hitting them.

## What is not here

See `internal_docs/deferred.md` under Phase 5.10. Chiefly: nothing in the game puts an enchantment
on anything — the table, the anvil and the grindstone are all menus, and there are no menus.
