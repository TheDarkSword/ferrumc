//! What the world does to whatever is standing in it, and what a blow comes to.
//!
//! Two halves. The first reads where something is and turns that into blows: how far it has fallen
//! since it last touched ground, how long it has held its breath, whether it is standing in fire or
//! lava, whether it has dropped out of the world. The second takes any blow, from here or from a
//! fight, and works out what actually lands.
//!
//! The arithmetic for both is in `ferrumc_damage`, which knows nothing about the world. What is
//! here is the two questions that need the world — what block is at its feet, and what block is at
//! its eyes — and the packets that follow.

use bevy_ecs::prelude::*;
use bevy_math::IVec3;
use ferrumc_attributes::Attributes;
use ferrumc_components::health::Health;
use ferrumc_components::player::hunger::Hunger;
use ferrumc_core::identity::entity_identity::EntityIdentity;
use ferrumc_core::identity::player_identity::PlayerIdentity;
use ferrumc_core::transform::grounded::OnGround;
use ferrumc_core::transform::position::Position;
use ferrumc_damage::vitals::{
    Vitals, FIRE_BURNS_FOR, FIRE_DAMAGE, LAVA_BURNS_FOR, LAVA_DAMAGE, SAFE_FALL, VOID_BELOW,
    VOID_DAMAGE,
};
use ferrumc_damage::{
    can_be_hurt, resolve, scale_for, Defence, Difficulty, Hit, Immunities, Reeling,
};
use ferrumc_data::attributes::Attribute;
use ferrumc_data::generated::damage_types::DamageType;
use ferrumc_data::generated::effects::Effect;
use ferrumc_effects::ActiveEffects;
use ferrumc_entities::components::Tracked;
use ferrumc_entities::entity_type::EntityType;
use ferrumc_entities::synced_data::{fields, EntityFlag, SyncedData};
use ferrumc_macros::match_block;
use ferrumc_messages::{EntityDamaged, EntityDied};
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::damage_event::DamageEventPacket;
use ferrumc_net::packets::outgoing::player_combat_kill::PlayerCombatKillPacket;
use ferrumc_net::packets::outgoing::remove_entities::RemoveEntitiesPacket;
use ferrumc_net::packets::outgoing::set_health::SetHealth;
use ferrumc_state::{GlobalState, GlobalStateResource};
use ferrumc_text::{ComponentBuilder, TextComponent};
use ferrumc_world::block_state_id::BlockStateId;
use ferrumc_world::pos::{ChunkBlockPos, ChunkPos};
use tracing::warn;

/// The bottom of the world. Everything below this is the void, once it is far enough below.
const WORLD_FLOOR: f64 = -64.0;

/// The furthest anything falls under its own weight in one tick.
///
/// Nothing accelerates past this, so a larger drop was not a fall: it was a teleport, a respawn or
/// a first tick with nothing to compare against. Vanilla clears what has been fallen on all three.
const FURTHEST_ONE_TICK_FALL: f64 = 4.0;

/// What is read off something to work out what the world is doing to it.
type InTheWorld<'a> = (
    Entity,
    &'a Position,
    &'a OnGround,
    &'a EntityType,
    &'a Health,
    &'a mut Vitals,
    &'a mut SyncedData,
    Option<&'a Attributes>,
);

/// Only things with health are asked. A dropped item carries the counters too, since everything
/// spawned does, but nothing can happen to it and asking the world twice about it every tick is two
/// chunk lookups for nothing.
type Alive = With<Health>;

/// Turns standing somewhere into being hurt by it.
pub fn hurt_by_the_world(
    mut entities: Query<InTheWorld, Alive>,
    state: Res<GlobalStateResource>,
    mut hurt: MessageWriter<EntityDamaged>,
) {
    for (entity, position, grounded, kind, health, mut vitals, mut data, attributes) in
        &mut entities
    {
        if health.current <= 0.0 {
            continue;
        }
        let feet = what_is_at(&state.0, position.coords.as_ivec3());
        let eyes = what_is_at(
            &state.0,
            (position.coords + bevy_math::DVec3::Y * f64::from(kind.eye_height())).as_ivec3(),
        );
        let mut blow = |what: DamageType, amount: f32| {
            hurt.write(EntityDamaged::from_the_world(entity, what, amount));
        };

        // Falling. Only the drop counts, and only out of water; landing settles the bill.
        let dropped = position.y - vitals.last_y;
        vitals.last_y = position.y;
        if dropped < -FURTHEST_ONE_TICK_FALL {
            vitals.fallen = 0.0;
        } else {
            vitals.fell(dropped, feet == Standing::In(Fluid::Water));
        }
        if grounded.0 {
            // How far is safe and how hard the landing lands are attributes, so feather falling
            // and a slow-falling potion move them rather than being special cases here.
            let (safe, multiplier) = attributes.map_or((SAFE_FALL, 1.0), |attributes| {
                (
                    attributes.value(&Attribute::SAFE_FALL_DISTANCE),
                    attributes.value(&Attribute::FALL_DAMAGE_MULTIPLIER),
                )
            });
            let fall = vitals.land(safe, multiplier);
            if fall > 0.0 {
                blow(DamageType::Fall, fall);
            }
        }

        // Burning, and what set it alight. Lava keeps something alight far longer than fire does,
        // and hurts it four times as hard while it stands there.
        match feet {
            Standing::In(Fluid::Lava) => {
                vitals.ignite(LAVA_BURNS_FOR);
                blow(DamageType::Lava, LAVA_DAMAGE);
            }
            Standing::InFire => {
                vitals.ignite(FIRE_BURNS_FOR);
                blow(DamageType::InFire, FIRE_DAMAGE);
            }
            _ => {}
        }
        let burning = vitals.burn(feet == Standing::In(Fluid::Lava));
        if burning > 0.0 {
            blow(DamageType::OnFire, burning);
        }

        // Breathing. What counts is where the eyes are, not where the feet are.
        // Respiration lets a tick of holding cost nothing, more often the higher it is. The roll
        // is vanilla's: an oxygen bonus of one skips half the ticks, of two skips two thirds.
        let oxygen =
            attributes.map_or(0.0, |attributes| attributes.value(&Attribute::OXYGEN_BONUS));
        let held_longer = oxygen > 0.0 && rand::random::<f64>() >= 1.0 / (oxygen + 1.0);
        let drowning = vitals.breathe(eyes == Standing::In(Fluid::Water), held_longer);
        if drowning > 0.0 {
            blow(DamageType::Drown, drowning);
        }

        // And the void, which nothing survives and nothing softens.
        if position.y < WORLD_FLOOR - f64::from(VOID_BELOW) {
            blow(DamageType::OutOfWorld, VOID_DAMAGE);
        }

        // Both of these are things a client draws rather than works out: the flames around an
        // entity and the row of bubbles above a player's hunger bar. Written here because this is
        // where they change, and sent by `synced_data::broadcast_changes` with everything else.
        data.set_flag(EntityFlag::OnFire, vitals.on_fire());
        data.set(fields::entity::AIR_SUPPLY, i32::from(vitals.air.max(0)));
    }
}

/// What something is standing in or on, as far as being hurt by it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    /// Nothing that hurts.
    Nothing,
    In(Fluid),
    InFire,
}

/// The two fluids that matter here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fluid {
    Water,
    Lava,
}

/// What is at a place, as far as being hurt by it goes.
fn what_is_at(state: &GlobalState, at: IVec3) -> Standing {
    let block = block_at(state, at);
    if match_block!("water", block) {
        Standing::In(Fluid::Water)
    } else if match_block!("lava", block) {
        Standing::In(Fluid::Lava)
    } else if match_block!("fire", block) || match_block!("soul_fire", block) {
        Standing::InFire
    } else {
        Standing::Nothing
    }
}

fn block_at(state: &GlobalState, pos: IVec3) -> BlockStateId {
    ferrumc_utils::world::load_or_generate_mut(state, ChunkPos::from(pos.as_dvec3()), "overworld")
        .expect("Failed to load or generate chunk")
        .get_block(ChunkBlockPos::from(pos))
}

/// What is read off a victim to work out what a blow comes to.
type Victim<'a> = (
    &'a mut Health,
    &'a mut Defence,
    &'a mut Reeling,
    &'a EntityType,
    Option<&'a Attributes>,
    Option<&'a ActiveEffects>,
    Option<&'a EntityIdentity>,
    Option<&'a StreamWriter>,
    Option<&'a Tracked>,
    Option<&'a Hunger>,
);

/// Takes any blow, from the world or from a fight, and works out what actually lands.
pub fn apply_damage(
    mut blows: MessageReader<EntityDamaged>,
    mut victims: Query<Victim>,
    watchers: Query<&StreamWriter>,
    players: Query<(), With<PlayerIdentity>>,
    difficulty: Res<Difficulty>,
    mut died: MessageWriter<EntityDied>,
) {
    for blow in blows.read() {
        let Ok((
            mut health,
            mut defence,
            mut reeling,
            kind,
            attributes,
            effects,
            identity,
            writer,
            tracked,
            hunger,
        )) = victims.get_mut(blow.entity)
        else {
            continue;
        };
        if health.current <= 0.0 {
            continue;
        }

        // What a thing is immune to comes off its kind. The invulnerable flag is a player in
        // creative, which is Phase 5.3's to read off the gamemode; fire and falling are the kind's
        // own answer and are here.
        let immune = Immunities {
            invulnerable: false,
            // Being unburnable is either what the kind is or what it has drunk.
            fire: kind.fire_immune()
                || effects.is_some_and(|held| held.has(Effect::FireResistance)),
            falling: false,
        };
        if !can_be_hurt(blow.kind, immune) {
            continue;
        }

        // How hard a mob hits moves with the difficulty; a player's blow and the world's own
        // hazards do not. What is behind a blow is a mob when something is to blame for it and
        // that something is not a player.
        let by_a_mob = blow.cause.is_some_and(|cause| players.get(cause).is_err());
        let amount = scale_for(blow.amount, blow.kind, by_a_mob, *difficulty);
        // What armour is worn is an attribute rather than a count of pieces, so it is read fresh
        // each time: a piece put on between one blow and the next counts on the next.
        if let Some(attributes) = attributes {
            defence.armour = attributes.value(&Attribute::ARMOR) as f32;
            defence.toughness = attributes.value(&Attribute::ARMOR_TOUGHNESS) as f32;
        }
        defence.resistance = effects
            .and_then(|held| held.level(Effect::Resistance))
            .unwrap_or(0);

        let landed = resolve(
            Hit {
                kind: blow.kind,
                amount,
            },
            &mut defence,
            &mut reeling,
        );
        if !landed.landed() {
            continue;
        }

        health.current = (health.current - landed.health).max(0.0);

        // A player is told their own health outright; everyone else reads it off the metadata row,
        // which `synced_data::mirror_components` keeps in step with the component just changed.
        if let Some(writer) = writer {
            // The same packet carries the stomach, so what is sent has to be what is actually
            // there rather than a full one.
            let (food, saturation) = hunger.map_or((20, 5.0), |hunger| {
                (i32::from(hunger.level), hunger.saturation)
            });
            if let Err(err) =
                writer.send_packet_ref(&SetHealth::new(health.current, food, saturation))
            {
                warn!("could not tell a player they were hurt: {err:?}");
            }
        }

        // And everyone watching gets the flash and the tilt. Whoever is watching an entity is
        // already known; a player is not watched that way, so being hurt goes to everyone.
        if let Some(identity) = identity {
            let event = DamageEventPacket::from_the_world(identity.entity_id, blow.kind);
            let told = |writer: &StreamWriter| {
                if let Err(err) = writer.send_packet_ref(&event) {
                    warn!("could not tell a player something was hurt: {err:?}");
                }
            };
            match tracked {
                Some(tracked) => tracked
                    .seen_by
                    .iter()
                    .filter_map(|player| watchers.get(*player).ok())
                    .for_each(told),
                None => watchers.iter().for_each(told),
            }
        }

        if health.current <= 0.0 {
            died.write(EntityDied {
                entity: blow.entity,
                kind: blow.kind,
                cause: blow.cause,
            });
        }
    }
}

/// One tick passing on how long something is still hard to hit again.
pub fn tick_reeling(mut victims: Query<&mut Reeling>) {
    for mut reeling in &mut victims {
        if reeling.ticks > 0 {
            reeling.tick();
        }
    }
}

/// What is read off something that has died, to know who to tell and how.
type Departing<'a> = (
    Option<&'a EntityIdentity>,
    Option<&'a PlayerIdentity>,
    Option<&'a StreamWriter>,
    Option<&'a Tracked>,
);

/// What happens once something has run out of health.
///
/// A player is shown the death screen and left where they fell until they ask to come back; that
/// ask arrives as a client command and is answered in `packet_handlers`. Anything else is taken out
/// of the world and everyone watching is told.
pub fn something_died(
    mut deaths: MessageReader<EntityDied>,
    dead: Query<Departing>,
    watchers: Query<&StreamWriter>,
    mut commands: Commands,
) {
    for death in deaths.read() {
        let Ok((identity, player, writer, tracked)) = dead.get(death.entity) else {
            continue;
        };

        if let (Some(player), Some(writer)) = (player, writer) {
            // Vanilla writes the message from what dealt the killing blow, and from the fight it
            // happened during where there was one. Only the first half is knowable yet, so what
            // goes out is the plain form: "<name> fell from a high place".
            let message = ComponentBuilder::translate(
                format!("death.attack.{}", death.kind.message_id()),
                vec![TextComponent::from(player.username.as_str())],
            );
            let told = PlayerCombatKillPacket::new(player.short_uuid, message);
            if let Err(err) = writer.send_packet_ref(&told) {
                warn!("could not show a player the death screen: {err:?}");
            }
            continue;
        }

        if let Some(identity) = identity {
            let gone = RemoveEntitiesPacket::of(&[identity.entity_id]);
            let told = |writer: &StreamWriter| {
                if let Err(err) = writer.send_packet_ref(&gone) {
                    warn!("could not tell a player something has gone: {err:?}");
                }
            };
            match tracked {
                Some(tracked) => tracked
                    .seen_by
                    .iter()
                    .filter_map(|player| watchers.get(*player).ok())
                    .for_each(told),
                None => watchers.iter().for_each(told),
            }
        }
        commands.entity(death.entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::message::{MessageRegistry, Messages};
    use bevy_ecs::schedule::Schedule;
    use ferrumc_damage::INVULNERABLE_TICKS;

    /// A world holding one thing that can be hurt, and the system that hurts it.
    fn a_world_with_something_in_it(health: f32, defence: Defence) -> (World, Entity, Schedule) {
        let mut world = World::new();
        MessageRegistry::register_message::<EntityDamaged>(&mut world);
        MessageRegistry::register_message::<EntityDied>(&mut world);
        world.insert_resource(Difficulty::default());

        let victim = world
            .spawn((
                Health {
                    current: health,
                    max: health,
                },
                defence,
                Reeling::default(),
                EntityType::Zombie,
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_damage);
        (world, victim, schedule)
    }

    fn hurt(world: &mut World, victim: Entity, kind: DamageType, amount: f32) {
        world
            .resource_mut::<Messages<EntityDamaged>>()
            .write(EntityDamaged::from_the_world(victim, kind, amount));
    }

    fn health_of(world: &World, victim: Entity) -> f32 {
        world.get::<Health>(victim).expect("it has health").current
    }

    #[test]
    fn a_blow_takes_the_health_off() {
        let (mut world, victim, mut schedule) =
            a_world_with_something_in_it(20.0, Defence::default());
        hurt(&mut world, victim, DamageType::Fall, 7.0);
        schedule.run(&mut world);
        assert_eq!(health_of(&world, victim), 13.0);
    }

    #[test]
    fn armour_softens_it_on_the_way_through() {
        let (mut world, victim, mut schedule) = a_world_with_something_in_it(
            20.0,
            Defence {
                armour: 20.0,
                ..Defence::default()
            },
        );
        hurt(&mut world, victim, DamageType::PlayerAttack, 10.0);
        schedule.run(&mut world);
        assert!(
            health_of(&world, victim) > 13.0,
            "armour should have taken most of a ten-point blow"
        );
    }

    #[test]
    fn armour_is_no_help_against_falling() {
        // Falling goes around armour, which is the whole reason the tag exists.
        let (mut world, victim, mut schedule) = a_world_with_something_in_it(
            20.0,
            Defence {
                armour: 20.0,
                ..Defence::default()
            },
        );
        hurt(&mut world, victim, DamageType::Fall, 7.0);
        schedule.run(&mut world);
        assert_eq!(health_of(&world, victim), 13.0);
    }

    #[test]
    fn two_blows_in_one_tick_are_not_two_blows_worth() {
        let (mut world, victim, mut schedule) =
            a_world_with_something_in_it(20.0, Defence::default());
        hurt(&mut world, victim, DamageType::Fall, 5.0);
        hurt(&mut world, victim, DamageType::Fall, 3.0);
        schedule.run(&mut world);
        assert_eq!(
            health_of(&world, victim),
            15.0,
            "the weaker second blow should have been swallowed"
        );
    }

    #[test]
    fn something_that_cannot_burn_is_not_burnt() {
        let mut world = World::new();
        MessageRegistry::register_message::<EntityDamaged>(&mut world);
        MessageRegistry::register_message::<EntityDied>(&mut world);
        world.insert_resource(Difficulty::default());
        let victim = world
            .spawn((
                Health {
                    current: 20.0,
                    max: 20.0,
                },
                Defence::default(),
                Reeling::default(),
                EntityType::Blaze,
            ))
            .id();
        let mut schedule = Schedule::default();
        schedule.add_systems(apply_damage);

        hurt(&mut world, victim, DamageType::OnFire, 1.0);
        schedule.run(&mut world);
        assert_eq!(health_of(&world, victim), 20.0);
    }

    #[test]
    fn running_out_of_health_says_so() {
        let (mut world, victim, mut schedule) =
            a_world_with_something_in_it(5.0, Defence::default());
        hurt(&mut world, victim, DamageType::OutOfWorld, 4.0);
        schedule.run(&mut world);
        assert_eq!(health_of(&world, victim), 1.0);
        assert!(world.resource::<Messages<EntityDied>>().is_empty());

        // Once the reeling has passed, the next blow finishes it.
        schedule.add_systems(tick_reeling);
        for _ in 0..INVULNERABLE_TICKS {
            schedule.run(&mut world);
        }
        hurt(&mut world, victim, DamageType::OutOfWorld, 4.0);
        schedule.run(&mut world);
        assert_eq!(health_of(&world, victim), 0.0);
        assert!(!world.resource::<Messages<EntityDied>>().is_empty());
    }
}
