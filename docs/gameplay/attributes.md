# Attributes

Every number an entity has that something else can change: health, speed, armour, reach, how hard it
hits, how far it falls before that hurts. `ferrumc-attributes` holds the arithmetic;
`src/bin/src/systems/attributes.rs` keeps it in step with what is worn.

## A base plus a stack of modifiers

Nothing writes an attribute directly. Putting on a helmet **adds a modifier** named after it, taking
it off **removes that name**, and the base is never touched — which is what makes taking it off
exact rather than nearly right.

```rust
attributes.value(&Attribute::ARMOR)              // 0.0
attributes.add(&Attribute::ARMOR, Modifier::known("head/minecraft:armor.helmet", 3.0, AddValue));
attributes.value(&Attribute::ARMOR)              // 3.0
attributes.remove_by_prefix("head/");
attributes.value(&Attribute::ARMOR)              // 0.0, exactly
```

Adding a modifier under a name that is already there **replaces** it. That is what stops
re-equipping the same boots doubling their armour.

## The order is the whole of it

```
base = born_with + Σ add_value
total = base + Σ (base × add_multiplied_base)
total = total × Π (1 + add_multiplied_total)
```

The middle step takes a share of the base **after** the flat amounts, not of what the entity was
born with. Two modifiers each adding a tenth of the base add a tenth of the *same* number — they do
not compound. Two multiplying the total do.

Applying them in any other order gives a different answer for the same set.

## Where the numbers come from

| what | where |
|---|---|
| the attribute itself: id, default, range, syncable | `scripts/extract_attributes.py` |
| what each kind of entity is born with | `scripts/extract_default_attributes.py` |
| what an item changes | the game's per-item component report |

None of it is transcribed. A zombie's twenty health, movement speed of 0.23 and follow range of 35
are built in code by the entity class rather than written in data, so they are asked of the game.

93 of the 158 kinds have attributes at all. An arrow, a boat and a dropped item have none, and that
is a real answer rather than an error.

## Slots

`head`, `chest`, `legs`, `feet`, `mainhand`, `offhand`. An item's modifier says which one it works
in, and may name a group instead: `armor` covers the four pieces, `hand` covers both hands, `any`
covers everything.

A modifier's name carries its slot in front of it, so the same item held in both hands stays two
modifiers. That prefix is folded into the path before it goes on the wire — a client reads a
modifier name as a resource location and disconnects if it cannot.

## Telling the client

`update_attributes` carries the **base and the modifiers separately**, not the total. That is what
lets a client draw the attack cooldown bar from `attack_speed` and show where a number came from.

The attribute registry grew four times across the supported versions (31 → 32 → 35 → 40) and was
renamed once: before 1.21.2 each attribute carried its group in front of it, `generic.armor` rather
than `armor`. The remap tables follow that rename **exactly** rather than guessing at a stand-in —
a stand-in item is a wrong icon, but a stand-in attribute would apply a modifier to the wrong number.

An attribute a client has never heard of is left out of the list, and the count is written after the
dropping. A length that does not match what follows is how a client ends up reading the rest of the
stream as attribute names.

## Who reads them

Armour and toughness feed the damage pipeline; `knockback_resistance` feeds the push;
`safe_fall_distance` and `fall_damage_multiplier` decide what landing costs; `oxygen_bonus` decides
how often a tick underwater costs no air; `attack_damage`, `attack_speed`, `attack_knockback` and
`sweeping_damage_ratio` decide what a swing is worth; `max_health` is the health ceiling.

## What is not here

See `internal_docs/deferred.md` under Phase 5.3. The short version: effects and enchantments add no
modifiers yet, `/attribute` does not exist, the physics still reads speed and gravity off the entity
type rather than the attributes, and a player's base values are not saved.

# Status effects

What a potion does, in `ferrumc-effects`, ticked by `src/bin/src/systems/effects.rs`.

## Most of an effect is just attribute modifiers

Twelve of the forty move a number and nothing else: speed moves `movement_speed`, strength moves
`attack_damage`, health boost moves `max_health`. Those are put on when the effect lands and taken
off when it runs out, and the attribute system does the rest without being asked.

The amount an effect carries is what **one level** is worth, so speed II is exactly twice speed I
rather than a separate modifier.

## Five do something on a tick

| effect | how often | what |
|---|---|---|
| regeneration | every `50 >> level` ticks | heals 1 |
| poison | every `25 >> level` ticks | hurts 1, never to death |
| wither | every `40 >> level` ticks | hurts 1, which does kill |
| hunger | every tick | `0.005 × level` exhaustion |

The interval halves with each level, so at level six and above they act every tick.

## Three land all at once and are never held

Healing (`4 << level`), harming (`6 << level`) and saturation. They do their work on being applied
and are gone. Absorption is held but does its work once — it tops up the extra health and then lasts
exactly as long as that health does.

## Applying the same effect twice

A stronger but shorter application does not replace a weaker longer one — it **hides** it, and the
weaker one comes back when the stronger runs out. Drinking a splash of swiftness II over a long
swiftness I leaves the swiftness I still running afterwards.

The chain is a stack, so three applications come back strongest first. It is flat rather than a
chain of boxes, which is also what lets it be written out with a player.

## What is not here

See `internal_docs/deferred.md` under Phase 5.4. Chiefly: nothing applies an effect yet — no potion
is drinkable, no beacon exists, and there is no `/effect`.
