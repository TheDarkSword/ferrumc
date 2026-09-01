//! What a broken block leaves behind, and what happens to it.
//!
//! Breaking a block asks the loot tables what it drops and puts that on the ground. From there it
//! is an entity like any other: it falls, it is tracked, it is written out with its chunk. What is
//! here is only what makes it a dropped thing rather than a mob — it waits, it joins its
//! neighbours, and it goes into whoever walks over it.
//!
//! Experience is the same shape with different arithmetic: an amount breaks into several orbs, and
//! an orb is pulled towards whoever is nearest rather than waiting to be walked over.

use bevy_ecs::prelude::*;
use ferrumc_core::identity::entity_identity::EntityIdentity;
use ferrumc_core::identity::player_identity::PlayerIdentity;
use ferrumc_core::tick::TickCounter;
use ferrumc_core::transform::grounded::OnGround;
use ferrumc_core::transform::position::Position;
use ferrumc_core::transform::rotation::Rotation;
use ferrumc_core::transform::velocity::Velocity;
use ferrumc_entities::components::Tracked;
use ferrumc_entities::drops::{
    pull_towards, DroppedItem, ExperienceOrb, MAX_STACK, MERGE_INTERVAL, MERGE_REACH, PICKUP_REACH,
};
use ferrumc_entities::entity_type::EntityType;
use ferrumc_entities::markers::HasCollisions;
use ferrumc_entities::synced_data::SyncedData;
use ferrumc_inventories::inventory::Inventory;
use ferrumc_inventories::item::ItemID;
use ferrumc_inventories::slot::InventorySlot;
use ferrumc_messages::{BlockBrokenEvent, EntityDied};
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::remove_entities::RemoveEntitiesPacket;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_predicates::context::{LootContext, LootParams, Origin};
use tracing::error;

use crate::systems::datapacks::Datapacks;

/// How high above the block a drop appears, and how far it may drift.
const DROP_HEIGHT: f64 = 0.5;

/// Puts what a broken block leaves behind on the ground.
///
/// What that is comes from the block's own loot table, so a stone that was mined without a pickaxe
/// leaves nothing and one that was mined with silk touch leaves itself.
pub fn drop_what_a_block_left(
    mut broken: MessageReader<BlockBrokenEvent>,
    packs: Res<Datapacks>,
    mut commands: Commands,
) {
    let mut rng = rand::thread_rng();
    for event in broken.read() {
        let Some(name) = block_name(event.state) else {
            continue;
        };
        let table = format!("minecraft:blocks/{name}");

        let mut context = LootContext::new(
            LootParams {
                origin: Some(Origin {
                    x: f64::from(event.position.pos.x) + 0.5,
                    y: f64::from(event.position.pos.y) + 0.5,
                    z: f64::from(event.position.pos.z) + 0.5,
                }),
                block_state: Some(event.state),
                ..LootParams::default()
            },
            &mut rng,
        );
        context.predicates = Some(&packs.predicates);

        let Ok(table) = ferrumc_datapack::Identifier::parse(&table) else {
            continue;
        };
        for stack in packs.loot.roll(&table, &mut context) {
            let slot = InventorySlot {
                item_id: Some(ItemID(VarInt(stack.item))),
                count: VarInt(stack.count),
                ..Default::default()
            };
            spawn_drop(
                &mut commands,
                DroppedItem::new(slot),
                f64::from(event.position.pos.x) + 0.5,
                f64::from(event.position.pos.y) + DROP_HEIGHT,
                f64::from(event.position.pos.z) + 0.5,
            );
        }
    }
}

/// Puts what a killed thing leaves behind on the ground.
///
/// Same machinery as a broken block: the loot table decides, and what it decides depends on what
/// killed it. Whether a player was holding a sword with looting on it is Phase 5.10's; for now a
/// mob drops what it drops.
pub fn drop_what_a_mob_left(
    mut deaths: MessageReader<EntityDied>,
    dead: Query<(&Position, &EntityType), Without<PlayerIdentity>>,
    packs: Res<Datapacks>,
    mut commands: Commands,
) {
    let mut rng = rand::thread_rng();
    for death in deaths.read() {
        let Ok((at, kind)) = dead.get(death.entity) else {
            continue;
        };
        let table = format!("minecraft:entities/{}", kind.name());
        let Ok(table) = ferrumc_datapack::Identifier::parse(&table) else {
            continue;
        };

        let mut context = LootContext::new(
            LootParams {
                origin: Some(Origin {
                    x: at.x,
                    y: at.y,
                    z: at.z,
                }),
                ..LootParams::default()
            },
            &mut rng,
        );
        context.predicates = Some(&packs.predicates);

        for stack in packs.loot.roll(&table, &mut context) {
            let slot = InventorySlot {
                item_id: Some(ItemID(VarInt(stack.item))),
                count: VarInt(stack.count),
                ..Default::default()
            };
            spawn_drop(&mut commands, DroppedItem::new(slot), at.x, at.y, at.z);
        }
    }
}

/// Puts one dropped item in the world, with everything an entity needs to be seen and to fall.
fn spawn_drop(commands: &mut Commands, dropped: DroppedItem, x: f64, y: f64, z: f64) {
    let at = Position::from(bevy_math::DVec3::new(x, y, z));
    let mut data = SyncedData::new(EntityType::Item);
    data.set(
        ferrumc_entities::synced_data::fields::item_entity::ITEM,
        dropped.stack.clone(),
    );
    commands.spawn((
        EntityIdentity::new(),
        EntityType::Item,
        at,
        Rotation::default(),
        Velocity::zero(),
        OnGround(false),
        HasCollisions,
        Tracked::starting_at(at.coords, 0.0, 0.0, false),
        data,
        dropped,
    ));
}

/// Ages what is lying about, and takes away what has waited too long.
pub fn age_what_is_lying_about(
    mut items: Query<(Entity, &mut DroppedItem)>,
    mut orbs: Query<(Entity, &mut ExperienceOrb)>,
    watchers: Query<&StreamWriter>,
    ids: Query<&EntityIdentity>,
    mut commands: Commands,
) {
    for (entity, mut dropped) in &mut items {
        dropped.age += 1;
        dropped.pickup_delay = dropped.pickup_delay.saturating_sub(1);
        if dropped.expired() {
            forget(entity, &ids, &watchers, &mut commands);
        }
    }
    for (entity, mut orb) in &mut orbs {
        orb.age += 1;
        if orb.expired() {
            forget(entity, &ids, &watchers, &mut commands);
        }
    }
}

/// Joins dropped items that have come to rest beside each other.
///
/// A floor covered in cobblestone is otherwise a thousand entities, each tracked and each written
/// out with its chunk.
pub fn merge_what_is_lying_about(
    // One query, read first and written after. Two queries over the same component in one system
    // is an access conflict, and Bevy stops the whole schedule for it — which takes the tick
    // thread with it and leaves a server that accepts connections and never answers.
    mut items: Query<(Entity, &Position, &mut DroppedItem)>,
    watchers: Query<&StreamWriter>,
    ids: Query<&EntityIdentity>,
    tick: Res<TickCounter>,
    mut commands: Commands,
) {
    if !tick.get().is_multiple_of(MERGE_INTERVAL) {
        return;
    }

    let lying: Vec<(Entity, bevy_math::DVec3, Option<i32>, i32)> = items
        .iter()
        .filter(|(_, _, dropped)| dropped.will_merge())
        .map(|(entity, at, dropped)| {
            (
                entity,
                at.coords,
                dropped.stack.item_id.map(|id| id.0 .0),
                dropped.stack.count.0,
            )
        })
        .collect();

    let mut taken: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    for (i, (into, at, kind, _)) in lying.iter().enumerate() {
        if taken.contains(into) {
            continue;
        }
        for (from, other_at, other_kind, other_count) in lying.iter().skip(i + 1) {
            if taken.contains(from) || kind != other_kind {
                continue;
            }
            if at.distance(*other_at) > MERGE_REACH {
                continue;
            }
            // Only as much as the stack will hold; whatever is left stays where it is.
            let Ok((_, _, mut growing)) = items.get_mut(*into) else {
                continue;
            };
            let room = i32::from(MAX_STACK) - growing.stack.count.0;
            if room <= 0 {
                break;
            }
            let moved = room.min(*other_count);
            growing.stack.count.0 += moved;

            let Ok((_, _, mut shrinking)) = items.get_mut(*from) else {
                continue;
            };
            shrinking.stack.count.0 -= moved;
            if shrinking.stack.count.0 <= 0 {
                taken.insert(*from);
                forget(*from, &ids, &watchers, &mut commands);
            }
        }
    }
}

/// Pulls orbs towards whoever is nearest.
pub fn pull_orbs_to_players(
    mut orbs: Query<(&Position, &mut Velocity), With<ExperienceOrb>>,
    players: Query<&Position, With<PlayerIdentity>>,
) {
    for (at, mut velocity) in &mut orbs {
        let nearest = players.iter().min_by(|a, b| {
            a.coords
                .distance(at.coords)
                .total_cmp(&b.coords.distance(at.coords))
        });
        let Some(player) = nearest else {
            continue;
        };
        // Towards the middle of a player rather than their feet, so an orb does not skim the floor.
        let eyes = player.coords + bevy_math::DVec3::new(0.0, 0.9, 0.0);
        **velocity += pull_towards(at.coords, eyes);
    }
}

/// Puts into a player whatever they have walked over.
pub fn pick_up_what_is_walked_over(
    items: Query<(Entity, &Position, &DroppedItem)>,
    orbs: Query<(Entity, &Position, &ExperienceOrb)>,
    mut players: Query<(Entity, &Position, &mut Inventory), With<PlayerIdentity>>,
    watchers: Query<&StreamWriter>,
    ids: Query<&EntityIdentity>,
    mut commands: Commands,
) {
    for (player, at, mut inventory) in &mut players {
        for (entity, lying, dropped) in &items {
            if !dropped.can_be_taken() || lying.coords.distance(at.coords) > PICKUP_REACH {
                continue;
            }
            if inventory.add_item(dropped.stack.clone()).is_ok() {
                forget(entity, &ids, &watchers, &mut commands);
            }
        }
        // Experience goes into a player rather than into their inventory; where it goes from there
        // is Phase 5's, so for now taking it only takes it off the ground.
        for (entity, lying, _) in &orbs {
            if lying.coords.distance(at.coords) <= PICKUP_REACH {
                forget(entity, &ids, &watchers, &mut commands);
            }
        }
        let _ = player;
    }
}

/// Takes something out of the world and tells everyone who could see it.
fn forget(
    entity: Entity,
    ids: &Query<&EntityIdentity>,
    watchers: &Query<&StreamWriter>,
    commands: &mut Commands,
) {
    if let Ok(identity) = ids.get(entity) {
        let gone = RemoveEntitiesPacket::of(&[identity.entity_id]);
        for writer in watchers {
            if let Err(err) = writer.send_packet_ref(&gone) {
                error!("could not tell a player something has gone: {err:?}");
            }
        }
    }
    commands.entity(entity).despawn();
}

/// The name a block state's loot table is filed under.
fn block_name(state: ferrumc_world::block_state_id::BlockStateId) -> Option<String> {
    let data = state.to_block_data()?;
    Some(
        data.name
            .rsplit(':')
            .next()
            .unwrap_or(&data.name)
            .to_string(),
    )
}
