//! Hitting something.
//!
//! The arithmetic is in `ferrumc_damage::combat`, which knows nothing about the world. What is here
//! is reading the attacker's state off their components, finding what they hit, and turning the
//! answer into a blow, a push, and — for a sword swung at a walk — the same again for everything
//! standing beside it.

use bevy_ecs::prelude::*;
use bevy_math::Vec2;
use ferrumc_attributes::Attributes;
use ferrumc_components::player::hunger::Hunger;
use ferrumc_core::identity::entity_identity::EntityIdentity;
use ferrumc_core::identity::player_identity::PlayerIdentity;
use ferrumc_core::transform::grounded::OnGround;
use ferrumc_core::transform::position::Position;
use ferrumc_core::transform::rotation::Rotation;
use ferrumc_core::transform::velocity::Velocity;
use ferrumc_damage::combat::{
    swing, Swinging, Target, Weapon, SWEEP_KNOCKBACK, SWEEP_RANGE_SQUARED, SWEEP_REACH,
    SWEEP_REACH_VERTICAL, SWING_EXHAUSTION,
};
use ferrumc_damage::{Swing, Vitals};
use ferrumc_data::attributes::Attribute;
use ferrumc_data::generated::damage_types::DamageType;
use ferrumc_data::generated::items::Item;
use ferrumc_entities::entity_type::EntityType;
use ferrumc_entities::synced_data::{EntityFlag, SyncedData};
use ferrumc_inventories::hotbar::Hotbar;
use ferrumc_inventories::inventory::Inventory;
use ferrumc_messages::EntityDamaged;
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::set_entity_motion::SetEntityMotion;
use ferrumc_net::AttackEntityReceiver;
use ferrumc_net_codec::net_types::lp_vec3::LowPrecisionVec3;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_physics::{knockback, Footing, DEFAULT_BLOCK_FRICTION};
use tracing::warn;

/// How fast a player walks, in blocks a tick.
///
/// The `movement_speed` attribute in vanilla; nothing here has attributes yet, so a player walks at
/// the speed vanilla gives them and nothing changes it.
const WALKING_SPEED: f64 = 0.1;

/// Everything a client could name by a number: a mob by its entity id, a player by their own.
type Named<'a> = (
    Entity,
    Option<&'a EntityIdentity>,
    Option<&'a PlayerIdentity>,
);

/// What is read off whatever is being pushed, and who has to be told about it.
type Pushed<'a> = (
    &'a mut Velocity,
    &'a OnGround,
    Option<&'a Attributes>,
    Option<&'a EntityIdentity>,
    Option<&'a PlayerIdentity>,
);

/// What is read off the attacker to know what their swing is worth.
///
/// Deliberately not the attacker's velocity: a player's is not driven by the server, and reading it
/// here while the push below writes it is an access conflict Bevy stops the whole schedule for.
type Attacker<'a> = (
    &'a Position,
    &'a Rotation,
    &'a OnGround,
    &'a SyncedData,
    &'a Vitals,
    &'a Inventory,
    &'a Hotbar,
    Option<&'a Attributes>,
);

// A system's arguments are the state it needs, and this one needs the attacker, the target, who is
// standing nearby and who has to be told about it. Splitting it to shorten the list would only move
// the same state into a resource.
#[expect(clippy::too_many_arguments)]
pub fn handle(
    attacks: Res<AttackEntityReceiver>,
    attackers: Query<Attacker, With<PlayerIdentity>>,
    known: Query<Named>,
    // Positions of everything a sweep could catch, and whether it is something that lives.
    standing: Query<(Entity, &Position, &EntityType)>,
    mut pushed: Query<Pushed>,
    watchers: Query<&StreamWriter>,
    mut swings: Query<&mut Swing>,
    mut hunger: Query<&mut Hunger>,
    mut hurt: MessageWriter<EntityDamaged>,
) {
    for (packet, attacker) in attacks.0.try_iter() {
        let Ok((at, facing, grounded, data, vitals, inventory, hotbar, attributes)) =
            attackers.get(attacker)
        else {
            continue;
        };
        let Some(target) = find(packet.entity_id.0, &known) else {
            continue;
        };
        if target == attacker {
            continue;
        }

        let held = held_item(inventory, hotbar);
        // What the swing is worth is the attacker's own numbers, which whatever is in their hand
        // has already moved: `systems::attributes` puts an item's modifiers on when it is picked
        // up and takes them off when it is put down.
        let weapon = attributes.map_or_else(
            || Weapon::in_hand(held),
            |attributes| Weapon {
                attack_damage: attributes.value(&Attribute::ATTACK_DAMAGE),
                attack_speed: attributes.value(&Attribute::ATTACK_SPEED),
                attack_knockback: attributes.value(&Attribute::ATTACK_KNOCKBACK),
                sweeping_ratio: attributes.value(&Attribute::SWEEPING_DAMAGE_RATIO),
            },
        );
        let charge = swings
            .get(attacker)
            .map_or(1.0, |swing| swing.charge(weapon.attack_speed));

        let living = standing
            .get(target)
            .map_or(true, |(_, _, kind)| kind.max_health().is_some());
        let blow = swing(
            weapon,
            charge,
            Swinging {
                sprinting: data.flag(EntityFlag::Sprinting),
                falling: vitals.fallen > 0.0,
                on_ground: grounded.0,
                in_water: false,
                on_a_ladder: false,
                riding: false,
                // How fast a player is actually going is theirs to report and nothing checks it,
                // so a swing is treated as taken at a standstill.
                speed: 0.0,
                walking_speed: attributes.map_or(WALKING_SPEED, |attributes| {
                    attributes.value(&Attribute::MOVEMENT_SPEED)
                }),
                holding_a_sword: is_a_sword(held),
            },
            Target { living },
        );

        hurt.write(EntityDamaged {
            entity: target,
            kind: DamageType::PlayerAttack,
            amount: blow.damage,
            cause: Some(attacker),
        });
        push(&mut pushed, &watchers, target, facing.yaw, blow.knockback);

        // A sweep catches everything standing around what was hit, for a share of the blow. The
        // thing that was hit is not caught twice, and neither is whoever swung.
        if let Some(share) = blow.sweep {
            let Ok((_, hit_at, _)) = standing.get(target) else {
                continue;
            };
            let nearby: Vec<Entity> = standing
                .iter()
                .filter(|(entity, beside, kind)| {
                    *entity != target
                        && *entity != attacker
                        && kind.max_health().is_some()
                        && (beside.x - hit_at.x).abs() <= SWEEP_REACH
                        && (beside.z - hit_at.z).abs() <= SWEEP_REACH
                        && (beside.y - hit_at.y).abs() <= SWEEP_REACH_VERTICAL
                        && beside.coords.distance_squared(at.coords) < SWEEP_RANGE_SQUARED
                })
                .map(|(entity, _, _)| entity)
                .collect();
            for bystander in nearby {
                hurt.write(EntityDamaged {
                    entity: bystander,
                    kind: DamageType::PlayerAttack,
                    amount: share,
                    cause: Some(attacker),
                });
                push(
                    &mut pushed,
                    &watchers,
                    bystander,
                    facing.yaw,
                    SWEEP_KNOCKBACK,
                );
            }
        }

        if let Ok(mut swing) = swings.get_mut(attacker) {
            swing.swung();
        }
        if let Ok(mut hunger) = hunger.get_mut(attacker) {
            hunger.exhaustion += SWING_EXHAUSTION;
        }
    }
}

/// One tick passing on how far everyone's attack has recharged.
pub fn tick_swings(mut swings: Query<&mut Swing>) {
    for mut swing in &mut swings {
        swing.tick();
    }
}

/// Which entity a client means by a number.
///
/// A player's number and a mob's are drawn from different ranges, so there is no ambiguity; both
/// are scanned because an attack is rare enough not to be worth an index.
fn find(id: i32, known: &Query<Named>) -> Option<Entity> {
    known
        .iter()
        .find(|(_, entity_id, player)| {
            entity_id.is_some_and(|identity| identity.entity_id == id)
                || player.is_some_and(|player| player.short_uuid == id)
        })
        .map(|(entity, _, _)| entity)
}

/// Pushes something away from whoever hit it.
///
/// Away from where the attacker is facing rather than from where they are standing, which is what
/// makes knockback aimable.
fn push(
    pushed: &mut Query<Pushed>,
    watchers: &Query<&StreamWriter>,
    target: Entity,
    yaw: f32,
    power: f32,
) {
    if power <= 0.0 {
        return;
    }
    let Ok((mut velocity, grounded, attributes, identity, player)) = pushed.get_mut(target) else {
        return;
    };
    let facing = Vec2::new(yaw.to_radians().sin(), -yaw.to_radians().cos());
    let footing = if grounded.0 {
        Footing::On(DEFAULT_BLOCK_FRICTION)
    } else {
        Footing::None
    };
    let resistance = attributes.map_or(0.0, |attributes| {
        attributes.value(&Attribute::KNOCKBACK_RESISTANCE) as f32
    });
    **velocity = knockback(**velocity, power, facing, resistance, footing);

    // A client drives its own player, so being pushed has to be sent to it or it simply will not
    // move. Everyone else reads the push off the position updates that follow.
    let id = identity
        .map(|identity| identity.entity_id)
        .or_else(|| player.map(|player| player.short_uuid));
    let Some(id) = id else { return };
    let shoved = SetEntityMotion {
        entity_id: VarInt::new(id),
        velocity: LowPrecisionVec3::new(
            f64::from(velocity.x),
            f64::from(velocity.y),
            f64::from(velocity.z),
        ),
    };
    for writer in watchers {
        if let Err(err) = writer.send_packet_ref(&shoved) {
            warn!("could not tell a player something was pushed: {err:?}");
        }
    }
}

/// What the attacker has in their main hand.
fn held_item(inventory: &Inventory, hotbar: &Hotbar) -> Option<&'static Item> {
    let slot = hotbar.get_selected_item(inventory).ok().flatten()?;
    let id = slot.item_id?;
    Item::from_id(u16::try_from(id.0 .0).ok()?)
}

/// Whether what is held sweeps.
///
/// Which items sweep is a tag the packs define rather than a list here, so a pack that adds a sword
/// gets a sweep without anything being changed.
fn is_a_sword(held: Option<&'static Item>) -> bool {
    let Some(held) = held else {
        return false;
    };
    let tags = ferrumc_registry::tags::current();
    let items = tags.item();
    let Some(swords) = items.get_by_name("minecraft:swords") else {
        warn!("the packs define no sword tag, so nothing sweeps");
        return false;
    };
    items.contains(swords, u32::from(held.id))
}
