# Damage

How much of a blow actually lands, and what puts a blow on the wire in the first place.

The arithmetic lives in `ferrumc-damage`, which touches neither the world nor the network. The two
systems that feed it are in `src/bin/src/systems/damage.rs`.

## A number of damage is not a number of health

A blow passes through four gates, in vanilla's order:

1. **Does it land at all.** Something just hit is briefly hard to hit again. For ten ticks after a
   blow, a new one only lands if it is *harder* than the last, and then only the difference lands —
   which is why two hits in the same moment are not two hits' worth.
2. **Armour**, unless the kind is in the packs' `bypasses_armor` group.
3. **Resistance**, unless the kind is in `bypasses_effects` or `bypasses_resistance`.
4. **Absorption**, which is spent before real health is.

## The armour formula is not the one people remember

Four per cent off per point is only true of a light blow. A heavy one cuts through:

```rust
let toughness = 2.0 + armour_toughness / 4.0;
let counts = (armour - damage / toughness).clamp(armour * 0.2, 20.0);
damage * (1.0 - counts / 25.0)
```

Twenty armour stops 78% of a one-point hit and far less of a forty-point one, which is why a fully
armoured player still dies to an anvil. Toughness is what holds the armour up against a heavy blow;
the floor at a fifth is what stops any blow getting through entirely.

## Which group a kind belongs to comes from the packs

`DamageType` is generated from `assets/extracted/26.2/data/minecraft/damage_type/`, and its
group predicates from the tag files beside it. Nothing is transcribed:

```rust
DamageType::Starve.goes_through_armour()   // true — the packs say so
DamageType::Fall.is_fall()                 // true
DamageType::Fall.message_id()              // "fall", for death.attack.fall
DamageType::MobAttack.scaling()            // Scaling::WhenCausedByLivingNonPlayer
DamageType::PlayerAttack.exhaustion()      // 0.1, for the hunger loop
```

A tag with nothing in it is a real answer rather than a mistake — vanilla keeps `bypasses_cooldown`
and puts nothing there — but a tag file that is missing entirely fails the build, because that means
the list has drifted from the packs.

## The number on the wire moves between versions

The damage type registry has grown four times across the ten supported versions: 47 kinds at 1.21,
49 at 1.21.8, 50 at 26.1, 51 at 26.2. A kind inserted in the middle shifts every kind after it, so
sending 26.2's number to a 1.21 client names a different kind of damage.

`DamageType::wire_id(version)` reads the place out of a per-version table generated from
`assets/data/registry_packets/`, which is the same file the client is actually sent — the two cannot
drift, because they are one file. A kind a version has never heard of returns `None`, and
`damage_event` falls back to `generic` rather than sending a place past the end of that client's
registry.

## What the world does

`hurt_by_the_world` runs once a tick over everything with health. It keeps three counters on
`Vitals` and turns each into a blow when it crosses a line:

| counter | what crosses | what it costs |
|---|---|---|
| `fallen` | landing, past 3 blocks | `floor(fallen + 1e-6 - 3)` |
| `air` | 300 ticks under water, then 20 more | 2 every tick after |
| `burning` | every twentieth tick alight | 1 |

Plus the two that are read straight off the block: standing in lava costs 4 a tick and sets
something alight for fifteen seconds, standing in fire costs 1 and sets it alight for eight.
Falling below the world floor less 64 costs 4 a tick and nothing softens it.

A drop larger than anything gravity can produce in one tick is not a fall — it is a teleport, a
respawn, or a first tick with nothing to compare against — and clears what was fallen, as vanilla
does on all three.

## Difficulty

Only what a mob does moves with the difficulty. Peaceful turns off mobs, not gravity: falling,
drowning and the void land the same at every setting.

```
peaceful  0
easy      min(damage / 2 + 1, damage)
normal    damage
hard      damage * 3 / 2
```

Set with `difficulty` in `config.toml`. Changing it while the server runs is not supported yet.

## Dying

Health reaching zero raises `EntityDied`. A player is shown the death screen with
`death.attack.<message_id>` and left where they fell until their client asks to come back; anything
else is taken out of the world and everyone watching is told.

Coming back is a `client_command` with `PerformRespawn`: full health, full hunger, lungs refilled,
nothing burning, and a place to stand on the surface above the origin — a world has no spawn point
to come back to yet.

## What is not here

See `internal_docs/deferred.md` under Phase 5.1. The short version: nothing carries armour yet
(attributes), resistance does nothing (effects), protection does nothing (enchantments), and no blow
has anyone to blame (combat) — so difficulty scaling never engages and no knockback is dealt.

# Combat

What a swing is worth, in `ferrumc-damage`'s `combat` module, wired up by
`src/bin/src/packet_handlers/play_packets/attack.rs`.

## A hit is not the weapon's damage

```
damage = weapon_damage * (0.2 + charge² * 0.8)   the recharge
       * 1.5                                      if it was a critical
```

The charge curve is the part worth knowing: **half recharged is 40% of the blow, not half of it.**
Waiting for the last part of the bar is worth far more than the first.

A swing counts as full strength above 0.9 — vanilla lets the last twentieth go, so a swing timed by
eye still counts.

## What a weapon is worth comes off the item

Not a list of weapons. An item carries modifiers to whoever holds it, and a sword's six points of
damage and its slower recharge are two of them:

```rust
Weapon::in_hand(Item::from_registry_key("minecraft:diamond_sword"))
// attack_damage 7.0  — one from the arm plus six from the sword
// attack_speed  1.6  — four from the arm less 2.4 from the sword
```

Which means a datapack that adds a weapon gets the right numbers without anything here changing.

## Three kinds of swing, and they are exclusive

| | needs |
|---|---|
| **knockback hit** | sprinting, full strength |
| **critical** | full strength, falling, feet off the ground, not in water, not on a ladder, not riding, not sprinting, target lives |
| **sweep** | full strength, not a critical, not a knockback hit, on the ground, holding a sword, moving no faster than a walk |

A critical adds half again. A knockback hit adds 0.5 to the push. A sweep catches everything within
a block of what was hit and three blocks of the attacker, for a share of the blow scaled by the
charge a second time.

Which items sweep is the packs' `#minecraft:swords` tag, not a list here.

## Knockback

`ferrumc_physics::knockback`, which is vanilla's: halve what the target already had, subtract the
push, and — only if the target is standing on something — lift it to at most 0.4.

The direction is where the attacker is *facing*, not where they are standing. That is what makes
knockback aimable.

A pushed player is sent `set_entity_motion`, because a client drives its own player and will
otherwise not move.

## What is not here

See `internal_docs/deferred.md` under Phase 5.2. The short version: shields do nothing (item-use
state), enchantments change nothing (5.10), the client draws a fist's cooldown bar for a sword
(attribute sync, 5.3), nothing is worth any experience, and only a player can swing.

# Hunger

Three numbers rather than the one a player sees, on the `Hunger` component and ticked by
`src/bin/src/systems/hunger.rs`.

| | visible | what it does |
|---|---|---|
| **exhaustion** | no | what actions cost; every 4 points spend 1 saturation |
| **saturation** | no | spent before the bar moves; also what drives fast healing |
| **food level** | yes | the shanks; only drops once saturation is gone |

That chain is why a full player can sprint a long way before a single shank moves.

## What costs anything

Sprinting 0.1 a block, swimming 0.01 a block, a jump 0.05, a sprinting jump 0.2, breaking a block
0.005, a swing 0.1, a slow heal 6.

**Walking and crouching cost nothing at all** — that is vanilla, not a gap.

## Healing runs off the same numbers, at two speeds

- Full (20) **and** with saturation left: heals `spent / 6` every 10 ticks, and pays `spent`
  exhaustion for it.
- Merely well fed (18+): heals 1 every 80 ticks, and pays 6.

## Starving stops where the difficulty says

An empty bar takes a point every 80 ticks, but only down to a floor: hard kills, normal leaves a
player on one heart, easy and peaceful on five.

## Eating

`PlayerEating` carries **only what was eaten**. What it is worth in food, in saturation and in
effects is the item's own answer, read in one place so the three cannot drift apart.

Thirteen items do something beyond feeding: a golden apple gives regeneration II and absorption,
milk clears everything, honey clears poison, rotten flesh makes a player hungry four times in five.
All of it comes off the item's `consumable` component.
