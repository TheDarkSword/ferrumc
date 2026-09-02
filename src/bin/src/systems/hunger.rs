//! What being hungry does to a player, once a tick.
//!
//! The arithmetic is on the [`Hunger`] component, which knows nothing about the world. What is here
//! is turning its answer into a heal, a blow or a packet — and the two places energy is spent that
//! the component cannot see for itself: moving, and being told about it.

use bevy_ecs::prelude::*;
use ferrumc_components::health::Health;
use ferrumc_components::player::hunger::{Fed, Hunger};
use ferrumc_core::identity::player_identity::PlayerIdentity;
use ferrumc_core::transform::position::Position;
use ferrumc_damage::Difficulty;
use ferrumc_data::generated::damage_types::DamageType;
use ferrumc_data::generated::effects::Effect;
use ferrumc_data::generated::items::{Aftermath, ConsumableImpl, DataComponent, FoodImpl, Item};
use ferrumc_effects::{ActiveEffects, Change, Instance};
use ferrumc_entities::synced_data::{EntityFlag, SyncedData};
use ferrumc_messages::EntityDamaged;
use ferrumc_messages::PlayerEating;
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::set_health::SetHealth;
use rand::Rng;

use crate::systems::effects::EffectChanged;
use tracing::warn;

/// What is read off a player to work out what being hungry costs them.
type Eating<'a> = (
    Entity,
    &'a mut Hunger,
    &'a mut Health,
    &'a StreamWriter,
    &'a SyncedData,
    &'a Position,
);

/// Where each player was last tick, so how far they moved is known.
///
/// A player's movement is their own client's to report, so the only way to know how far they went
/// is to remember where they were.
#[derive(Component, Debug, Clone, Copy)]
pub struct WasAt(pub bevy_math::DVec3);

/// One tick of being hungry.
pub fn tick_hunger(
    mut players: Query<Eating, With<PlayerIdentity>>,
    mut travelled: Query<&mut WasAt>,
    difficulty: Res<Difficulty>,
    mut hurt: MessageWriter<EntityDamaged>,
) {
    for (player, mut hunger, mut health, writer, data, at) in &mut players {
        let before = (hunger.level, hunger.saturation);

        // Moving is what spends most of a stomach, and walking spends none of it: only sprinting
        // and swimming cost anything at all.
        if let Ok(mut was) = travelled.get_mut(player) {
            let moved = at.coords.distance(was.0);
            was.0 = at.coords;
            hunger.travelled(
                moved,
                data.flag(EntityFlag::Sprinting),
                data.flag(EntityFlag::Swimming),
            );
        }

        let wounded = health.current < health.max;
        match hunger.tick(wounded, health.current, *difficulty) {
            Fed::Nothing => {}
            Fed::Heal(amount) => {
                health.current = (health.current + amount).min(health.max);
            }
            Fed::Starve => {
                hurt.write(EntityDamaged::from_the_world(
                    player,
                    DamageType::Starve,
                    1.0,
                ));
            }
        }

        // A client draws the shanks from what it was last told, so it is told again whenever they
        // would look different. Saturation is invisible but decides how the bar wobbles.
        if (hunger.level, hunger.saturation) != before {
            let told = SetHealth::new(health.current, i32::from(hunger.level), hunger.saturation);
            if let Err(err) = writer.send_packet_ref(&told) {
                warn!("could not tell a player how hungry they are: {err:?}");
            }
        }
    }
}

/// A player who has appeared but is not yet being followed.
type Arrived<'a> = (Entity, &'a Position);
type NotYetTracked = (With<PlayerIdentity>, Without<WasAt>);

/// Starts remembering where a player is, the tick after they appear.
pub fn remember_where_players_are(new: Query<Arrived, NotYetTracked>, mut commands: Commands) {
    for (player, at) in &new {
        commands.entity(player).insert(WasAt(at.coords));
    }
}

/// What is read off a player who has finished eating.
type Eater<'a> = (&'a mut Hunger, Option<&'a mut ActiveEffects>);

/// Feeds whoever has finished eating, and does to them whatever the food does.
///
/// What a thing is worth and what it does afterwards are both the item's own answer, read here
/// rather than worked out by whoever handed it over.
pub fn feed_whoever_ate(
    mut eaten: MessageReader<PlayerEating>,
    mut eaters: Query<Eater>,
    mut changed: MessageWriter<EffectChanged>,
) {
    let mut rng = rand::thread_rng();
    for meal in eaten.read() {
        let Some(item) = Item::from_id(u16::try_from(meal.item.0 .0).unwrap_or(u16::MAX)) else {
            continue;
        };
        let Ok((mut hunger, mut effects)) = eaters.get_mut(meal.player) else {
            continue;
        };

        if let Some(food) = component::<FoodImpl>(item, DataComponent::Food) {
            hunger.eat(food.nutrition, food.saturation);
        }

        let Some(consumable) = component::<ConsumableImpl>(item, DataComponent::Consumable) else {
            continue;
        };
        let Some(effects) = effects.as_mut() else {
            continue;
        };

        for after in consumable.after {
            // Some of it only happens sometimes: rotten flesh makes a player hungry four times in
            // five, and a roll that fails means nothing happened at all.
            if after.probability < 1.0 && rng.gen::<f32>() >= after.probability {
                continue;
            }
            match &after.what {
                Aftermath::Apply(applied) => {
                    for (name, amplifier, duration) in *applied {
                        let Some(effect) = Effect::from_name(name) else {
                            continue;
                        };
                        effects.add(effect, Instance::new(*amplifier, *duration));
                        changed.write(EffectChanged {
                            entity: meal.player,
                            effect,
                            change: Change::Applied,
                        });
                    }
                }
                Aftermath::Remove(named) => {
                    for name in *named {
                        let Some(effect) = Effect::from_name(name) else {
                            continue;
                        };
                        if effects.remove(effect) {
                            changed.write(EffectChanged {
                                entity: meal.player,
                                effect,
                                change: Change::Gone,
                            });
                        }
                    }
                }
                Aftermath::ClearEverything => {
                    for effect in effects.clear() {
                        changed.write(EffectChanged {
                            entity: meal.player,
                            effect,
                            change: Change::Gone,
                        });
                    }
                }
                // Moving the eater and making a noise both need more of the world than is here.
                Aftermath::TeleportRandomly | Aftermath::PlaySound => {}
            }
        }
    }
}

/// One of an item's components, where it has it.
fn component<T: 'static>(item: &'static Item, which: DataComponent) -> Option<&'static T> {
    item.components
        .iter()
        .find_map(|(id, data)| (*id == which).then(|| data.as_any().downcast_ref::<T>()))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::message::{MessageRegistry, Messages};
    use bevy_ecs::schedule::Schedule;
    use ferrumc_data::generated::items::Item;
    use ferrumc_inventories::item::ItemID;
    use ferrumc_net_codec::net_types::var_int::VarInt;

    /// A world holding one hungry thing, and the system that feeds it.
    fn a_hungry_world() -> (World, Entity, Schedule) {
        let mut world = World::new();
        MessageRegistry::register_message::<PlayerEating>(&mut world);
        MessageRegistry::register_message::<EffectChanged>(&mut world);

        let player = world
            .spawn((
                Hunger {
                    level: 10,
                    saturation: 0.0,
                    ..Hunger::default()
                },
                ActiveEffects::default(),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(feed_whoever_ate);
        (world, player, schedule)
    }

    fn eat(world: &mut World, player: Entity, what: &str) {
        let id = Item::from_registry_key(what).expect("it is an item").id;
        world
            .resource_mut::<Messages<PlayerEating>>()
            .write(PlayerEating {
                player,
                item: ItemID(VarInt::new(i32::from(id))),
            });
    }

    #[test]
    fn eating_a_steak_fills_a_stomach() {
        let (mut world, player, mut schedule) = a_hungry_world();
        eat(&mut world, player, "minecraft:cooked_beef");
        schedule.run(&mut world);

        let hunger = world.get::<Hunger>(player).expect("it has a stomach");
        assert_eq!(hunger.level, 18, "ten plus eight");
        assert!(hunger.saturation > 0.0);
    }

    #[test]
    fn a_golden_apple_does_what_a_golden_apple_does() {
        let (mut world, player, mut schedule) = a_hungry_world();
        eat(&mut world, player, "minecraft:golden_apple");
        schedule.run(&mut world);

        let effects = world
            .get::<ActiveEffects>(player)
            .expect("it can be affected");
        assert_eq!(
            effects.level(Effect::Regeneration),
            Some(2),
            "regeneration II"
        );
        assert_eq!(effects.level(Effect::Absorption), Some(1));
    }

    #[test]
    fn milk_takes_everything_away() {
        let (mut world, player, mut schedule) = a_hungry_world();
        let mut effects = world
            .get_mut::<ActiveEffects>(player)
            .expect("it can be affected");
        effects.add(Effect::Poison, Instance::new(0, 200));
        effects.add(Effect::Speed, Instance::new(0, 200));

        eat(&mut world, player, "minecraft:milk_bucket");
        schedule.run(&mut world);

        assert!(world
            .get::<ActiveEffects>(player)
            .expect("it can be affected")
            .is_empty());
    }

    #[test]
    fn honey_takes_away_only_poison() {
        let (mut world, player, mut schedule) = a_hungry_world();
        let mut effects = world
            .get_mut::<ActiveEffects>(player)
            .expect("it can be affected");
        effects.add(Effect::Poison, Instance::new(0, 200));
        effects.add(Effect::Speed, Instance::new(0, 200));

        eat(&mut world, player, "minecraft:honey_bottle");
        schedule.run(&mut world);

        let effects = world
            .get::<ActiveEffects>(player)
            .expect("it can be affected");
        assert!(!effects.has(Effect::Poison));
        assert!(effects.has(Effect::Speed), "and leaves the rest alone");
    }

    #[test]
    fn something_that_is_not_food_feeds_nobody() {
        let (mut world, player, mut schedule) = a_hungry_world();
        eat(&mut world, player, "minecraft:dirt");
        schedule.run(&mut world);
        assert_eq!(
            world.get::<Hunger>(player).expect("it has a stomach").level,
            10
        );
    }
}
