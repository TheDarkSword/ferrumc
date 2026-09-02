//! What a potion does to whoever drank it, once a tick.
//!
//! Most of an effect is a set of modifiers on the holder's attributes, and those are put on when it
//! is applied and taken off when it runs out — the attribute system does the rest without being
//! asked. What is here is the handful that do something else: regeneration heals, poison and wither
//! hurt, hunger and saturation move a stomach.
//!
//! The arithmetic is in `ferrumc_effects`, which knows nothing about the world.

use bevy_ecs::prelude::*;
use ferrumc_attributes::{Attributes, Modifier, Operation};
use ferrumc_components::health::Health;
use ferrumc_components::player::hunger::Hunger;
use ferrumc_core::identity::entity_identity::EntityIdentity;
use ferrumc_core::identity::player_identity::PlayerIdentity;
use ferrumc_core::tick::TickCounter;
use ferrumc_damage::Defence;
use ferrumc_data::attributes::Attribute;
use ferrumc_data::generated::damage_types::DamageType;
use ferrumc_data::generated::effects::{Effect, Operation as EffectOperation};
use ferrumc_effects::{ActiveEffects, Change, Tick};
use ferrumc_messages::EntityDamaged;
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::remove_mob_effect::RemoveMobEffect;
use ferrumc_net::packets::outgoing::update_mob_effect::{Shown, UpdateMobEffect};
use tracing::warn;

/// The most food a stomach holds.
const FULL_STOMACH: u8 = 20;

/// What is read off something under the influence.
type Affected<'a> = (
    Entity,
    &'a mut ActiveEffects,
    &'a mut Health,
    Option<&'a mut Hunger>,
    Option<&'a mut Defence>,
);

/// One tick passing on everything anyone is under the influence of.
pub fn tick_effects(
    mut affected: Query<Affected>,
    tick: Res<TickCounter>,
    mut hurt: MessageWriter<EntityDamaged>,
    mut told: MessageWriter<EffectChanged>,
) {
    // An endless effect keeps time by the world's clock rather than its own countdown, which is
    // vanilla's way of giving one a steady beat when it has no countdown to beat against.
    let age = i32::try_from(tick.get() % i64::from(i32::MAX) as u64).unwrap_or(0);

    for (entity, mut effects, mut health, mut hunger, mut defence) in &mut affected {
        if effects.is_empty() {
            continue;
        }
        let ferrumc_effects::Ticked {
            doing,
            told: changes,
        } = effects.tick(age);

        for (_, what) in doing {
            match what {
                Tick::Nothing => {}
                Tick::Heal(amount) => {
                    health.current = (health.current + amount).min(health.max);
                }
                Tick::Hurt { amount, spares } => {
                    // Poison never finishes anyone off; harming does.
                    if spares && health.current <= 1.0 {
                        continue;
                    }
                    hurt.write(EntityDamaged::from_the_world(
                        entity,
                        DamageType::Magic,
                        amount,
                    ));
                }
                Tick::Wither(amount) => {
                    hurt.write(EntityDamaged::from_the_world(
                        entity,
                        DamageType::Wither,
                        amount,
                    ));
                }
                Tick::Hunger(amount) => {
                    if let Some(hunger) = hunger.as_mut() {
                        hunger.exhaustion += amount;
                    }
                }
                Tick::Feed(amount) => {
                    if let Some(hunger) = hunger.as_mut() {
                        hunger.level = (hunger.level + amount).min(FULL_STOMACH);
                    }
                }
                Tick::Absorb(amount) => {
                    if let Some(defence) = defence.as_mut() {
                        defence.absorption = defence.absorption.max(amount);
                    }
                }
            }
        }

        // Absorption lasts exactly as long as the extra health it gave, so once that is spent the
        // effect goes with it.
        let spent = defence.is_some_and(|defence| defence.absorption <= 0.0);
        if spent && effects.has(Effect::Absorption) {
            effects.remove(Effect::Absorption);
            told.write(EffectChanged {
                entity,
                effect: Effect::Absorption,
                change: Change::Gone,
            });
        }

        for (effect, change) in changes {
            told.write(EffectChanged {
                entity,
                effect,
                change,
            });
        }
    }
}

/// An effect was applied to something, or has gone from it.
///
/// Raised rather than acted on directly because two things follow from it — the modifiers going on
/// or off, and a client being told — and both want the same list.
#[derive(Message)]
pub struct EffectChanged {
    pub entity: Entity,
    pub effect: Effect,
    pub change: Change,
}

/// Puts an effect's modifiers on the holder's numbers, and takes them off again.
///
/// What an effect moves is a set of modifiers on attributes rather than anything special: speed
/// moves `movement_speed`, strength moves `attack_damage`. The amount is what one level is worth,
/// so a level is a multiple of it.
pub fn apply_effect_modifiers(
    mut changes: MessageReader<EffectChanged>,
    holders: Query<&ActiveEffects>,
    mut numbers: Query<&mut Attributes>,
) {
    for change in changes.read() {
        let Ok(mut attributes) = numbers.get_mut(change.entity) else {
            continue;
        };
        for modifier in change.effect.modifiers() {
            let Some(attribute) = Attribute::from_name(modifier.attribute) else {
                continue;
            };
            match change.change {
                Change::Gone => {
                    attributes.remove(attribute, modifier.name);
                }
                Change::Applied => {
                    let Some(level) = holders
                        .get(change.entity)
                        .ok()
                        .and_then(|held| held.level(change.effect))
                    else {
                        continue;
                    };
                    attributes.add(
                        attribute,
                        Modifier {
                            name: modifier.name.into(),
                            amount: modifier.amount * f64::from(level),
                            operation: match modifier.operation {
                                EffectOperation::AddValue => Operation::AddValue,
                                EffectOperation::AddMultipliedBase => Operation::AddMultipliedBase,
                                EffectOperation::AddMultipliedTotal => {
                                    Operation::AddMultipliedTotal
                                }
                            },
                        },
                    );
                }
            }
        }
    }
}

/// What is read off something to say who a client should be told about.
type Named<'a> = (
    Option<&'a ActiveEffects>,
    Option<&'a EntityIdentity>,
    Option<&'a PlayerIdentity>,
);

/// Tells clients what has been applied and what has gone.
pub fn send_effect_changes(
    mut changes: MessageReader<EffectChanged>,
    known: Query<Named>,
    watchers: Query<&StreamWriter>,
) {
    for change in changes.read() {
        let Ok((effects, identity, player)) = known.get(change.entity) else {
            continue;
        };
        let id = identity
            .map(|identity| identity.entity_id)
            .or_else(|| player.map(|player| player.short_uuid));
        let Some(id) = id else { continue };

        let sent: Box<dyn Fn(&StreamWriter) -> bool> = match change.change {
            Change::Gone => {
                let gone = RemoveMobEffect::new(id, u32::from(change.effect.id()));
                Box::new(move |writer: &StreamWriter| writer.send_packet_ref(&gone).is_ok())
            }
            Change::Applied => {
                let Some(held) = effects.and_then(|effects| effects.get(change.effect)) else {
                    continue;
                };
                let applied = UpdateMobEffect::new(
                    id,
                    u32::from(change.effect.id()),
                    held.amplifier,
                    held.duration,
                    Shown {
                        ambient: held.ambient,
                        visible: held.visible,
                        show_icon: held.show_icon,
                        blend: false,
                    },
                );
                Box::new(move |writer: &StreamWriter| writer.send_packet_ref(&applied).is_ok())
            }
        };

        for writer in &watchers {
            if !sent(writer) {
                warn!("could not tell a player about an effect");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::message::{MessageRegistry, Messages};
    use bevy_ecs::schedule::Schedule;
    use ferrumc_core::tick::TickCounter;
    use ferrumc_effects::Instance;
    use ferrumc_entities::entity_type::EntityType;

    /// A world holding one thing that can be affected, and the systems that affect it.
    fn a_world() -> (World, Entity, Schedule) {
        let mut world = World::new();
        MessageRegistry::register_message::<EntityDamaged>(&mut world);
        MessageRegistry::register_message::<EffectChanged>(&mut world);
        world.insert_resource(TickCounter::new());

        let victim = world
            .spawn((
                ActiveEffects::default(),
                Attributes::for_entity(EntityType::Player.protocol_id()),
                Health::default(),
                Defence::default(),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems((tick_effects, apply_effect_modifiers).chain());
        (world, victim, schedule)
    }

    fn apply(world: &mut World, entity: Entity, effect: Effect, amplifier: u8, duration: i32) {
        let mut effects = world
            .get_mut::<ActiveEffects>(entity)
            .expect("it can be affected");
        effects.add(effect, Instance::new(amplifier, duration));
        world
            .resource_mut::<Messages<EffectChanged>>()
            .write(EffectChanged {
                entity,
                effect,
                change: Change::Applied,
            });
    }

    fn speed_of(world: &World, entity: Entity) -> f64 {
        world
            .get::<Attributes>(entity)
            .expect("it has numbers")
            .value(&Attribute::MOVEMENT_SPEED)
    }

    #[test]
    fn a_speed_potion_raises_movement_speed_and_running_out_puts_it_back() {
        let (mut world, player, mut schedule) = a_world();
        let walking = speed_of(&world, player);

        apply(&mut world, player, Effect::Speed, 0, 3);
        schedule.run(&mut world);
        let hurrying = speed_of(&world, player);
        assert!((hurrying - walking * 1.2).abs() < 1e-9, "a fifth faster");

        // And when it runs out, exactly back.
        for _ in 0..4 {
            schedule.run(&mut world);
        }
        assert_eq!(speed_of(&world, player), walking);
    }

    #[test]
    fn a_second_level_is_twice_a_first() {
        let (mut world, player, mut schedule) = a_world();
        let walking = speed_of(&world, player);

        apply(&mut world, player, Effect::Speed, 1, 100);
        schedule.run(&mut world);
        assert!(
            (speed_of(&world, player) - walking * 1.4).abs() < 1e-9,
            "two fifths, not two twentieths"
        );
    }

    #[test]
    fn strength_adds_a_flat_amount_rather_than_a_share() {
        let (mut world, player, mut schedule) = a_world();
        apply(&mut world, player, Effect::Strength, 0, 100);
        schedule.run(&mut world);

        let hitting = world
            .get::<Attributes>(player)
            .expect("it has numbers")
            .value(&Attribute::ATTACK_DAMAGE);
        assert_eq!(hitting, 1.0 + 3.0, "a fist plus what strength gives");
    }

    #[test]
    fn regeneration_heals_and_poison_hurts() {
        let (mut world, player, mut schedule) = a_world();
        world
            .get_mut::<Health>(player)
            .expect("it has health")
            .current = 10.0;

        apply(&mut world, player, Effect::Regeneration, 0, 60);
        for _ in 0..51 {
            schedule.run(&mut world);
        }
        let healed = world.get::<Health>(player).expect("it has health").current;
        assert!(
            healed > 10.0,
            "regeneration should have healed it: {healed}"
        );

        // Poison lands every twenty-fifth tick of what is left, so it takes a few before the
        // countdown reaches one.
        apply(&mut world, player, Effect::Poison, 0, 60);
        for _ in 0..11 {
            schedule.run(&mut world);
        }
        assert!(
            !world.resource::<Messages<EntityDamaged>>().is_empty(),
            "poison should have raised a blow"
        );
    }

    #[test]
    fn poison_never_finishes_anyone_off() {
        let (mut world, player, mut schedule) = a_world();
        world
            .get_mut::<Health>(player)
            .expect("it has health")
            .current = 1.0;

        apply(&mut world, player, Effect::Poison, 0, 100);
        for _ in 0..30 {
            schedule.run(&mut world);
        }
        assert!(
            world.resource::<Messages<EntityDamaged>>().is_empty(),
            "poison raised a blow against something on its last heart"
        );
    }

    #[test]
    fn absorption_gives_extra_health_once() {
        let (mut world, player, mut schedule) = a_world();
        let mut effects = world
            .get_mut::<ActiveEffects>(player)
            .expect("it can be affected");
        let (at_once, _) = effects.add(Effect::Absorption, Instance::new(1, 100));
        assert_eq!(at_once, ferrumc_effects::Tick::Absorb(8.0));

        // The effect itself does nothing more, so what it gave can be spent.
        schedule.run(&mut world);
        assert_eq!(
            world
                .get::<Defence>(player)
                .expect("it has defences")
                .absorption,
            0.0,
            "nothing tops it up behind the pipeline's back"
        );
    }
}
