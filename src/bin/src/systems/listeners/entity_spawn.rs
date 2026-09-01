use bevy_ecs::prelude::*;
use ferrumc_core::identity::entity_identity::EntityIdentity;
use ferrumc_core::transform::grounded::OnGround;
use ferrumc_core::transform::position::Position;
use ferrumc_core::transform::rotation::Rotation;
use ferrumc_core::transform::velocity::Velocity;
use ferrumc_entities::components::{CombatProperties, Tracked};
use ferrumc_entities::entity_type::EntityType;
use ferrumc_entities::markers::entity_types::*;
use ferrumc_entities::markers::HasCollisions;
use ferrumc_entities::synced_data::SyncedData;
use ferrumc_messages::{SpawnEntityCommand, SpawnEntityEvent};
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::entity_metadata::EntityMetadataPacket;
use ferrumc_net::packets::outgoing::spawn_entity::SpawnEntityPacket;
use ferrumc_net_codec::net_types::var_int::VarInt;
use tracing::{error, warn};

/// Helper function to broadcast entity spawn packets to all connected players.
///
/// This function queries the entity's components and sends the spawn packet
/// to all players. It's generic and works for any entity type.
///
/// # Arguments
///
/// * `world` - The Bevy world
/// * `entity` - The entity to broadcast
fn broadcast_entity_spawn(world: &mut World, entity: Entity) {
    // Get entity components
    let kind = match world.get::<EntityType>(entity) {
        Some(m) => m,
        None => {
            error!("Failed to get the entity type of {:?}", entity);
            return;
        }
    };
    let protocol_id = kind.protocol_id();

    let identity = match world.get::<EntityIdentity>(entity) {
        Some(i) => i,
        None => {
            error!("Failed to get entity identity for {:?}", entity);
            return;
        }
    };

    let position = match world.get::<Position>(entity) {
        Some(p) => p,
        None => {
            error!("Failed to get entity position for {:?}", entity);
            return;
        }
    };

    let rotation = match world.get::<Rotation>(entity) {
        Some(r) => r,
        None => {
            error!("Failed to get entity rotation for {:?}", entity);
            return;
        }
    };

    // Create spawn packet
    let spawn_packet = SpawnEntityPacket::new(
        identity.entity_id,
        identity.uuid.as_u128(),
        protocol_id as i32,
        position,
        rotation,
    );

    // What the entity looks like has to follow it, or it spawns as a default of its type.
    let metadata = world
        .get::<SyncedData>(entity)
        .map(|data| EntityMetadataPacket::everything(VarInt::new(identity.entity_id), data));

    // Broadcast to all connected players
    let mut writer_query = world.query::<&StreamWriter>();
    for writer in writer_query.iter(world) {
        if let Err(e) = writer.send_packet_ref(&spawn_packet) {
            error!("Failed to send spawn packet: {:?}", e);
        }
        if let Some(metadata) = &metadata {
            if let Err(e) = writer.send_packet_ref(metadata) {
                error!("Failed to send spawn metadata: {:?}", e);
            }
        }
    }
}

/// System that processes spawn commands from messages
pub fn spawn_command_processor(
    mut spawn_commands: MessageReader<SpawnEntityCommand>,
    query: Query<(&Position, &Rotation)>,
    mut spawn_events: MessageWriter<SpawnEntityEvent>,
) {
    // Process all spawn command messages
    for command in spawn_commands.read() {
        // Get player position and rotation
        if let Ok((pos, rot)) = query.get(command.player_entity) {
            // Calculate spawn position 2 blocks in front of the player
            let spawn_pos = pos.offset_forward(rot, 2.0);

            spawn_events.write(SpawnEntityEvent::fresh(command.entity_type, spawn_pos));
        } else {
            warn!(
                "Failed to get position for entity {:?}",
                command.player_entity
            );
        }
    }
}

/// System that listens for `SpawnEntityEvent` and spawns the entity,
/// then broadcasts the spawn packet.
pub fn handle_spawn_entity(mut events: MessageReader<SpawnEntityEvent>, mut commands: Commands) {
    for event in events.read() {
        let kind = event.entity_type;
        // One coming back keeps the name it had; one appearing for the first time is given one.
        let identity = event
            .uuid
            .map_or_else(EntityIdentity::new, EntityIdentity::with_uuid);
        let mut entity = commands.spawn((
            identity,
            kind,
            CombatProperties::default(),
            event.position,
            Rotation::default(),
            Velocity::zero(),
            OnGround(false),
            Tracked::starting_at(event.position.coords, 0.0, 0.0, false),
            HasCollisions,
            SyncedData::new(kind),
        ));

        // The two kinds that are more than where they are: one carries a stack, the other an
        // amount. Spawning either by hand makes a real one rather than an empty shell.
        match kind {
            EntityType::Item => {
                entity.insert(ferrumc_entities::drops::DroppedItem::new(
                    ferrumc_inventories::slot::InventorySlot::empty(),
                ));
            }
            EntityType::ExperienceOrb => {
                entity.insert(ferrumc_entities::drops::ExperienceOrb::new(1));
            }
            _ => {}
        }

        // The marker a system may filter an archetype on, for the kinds that have one.
        match kind {
            EntityType::Allay => entity.insert(Allay),
            EntityType::Armadillo => entity.insert(Armadillo),
            EntityType::Axolotl => entity.insert(Axolotl),
            EntityType::Bat => entity.insert(Bat),
            EntityType::Bee => entity.insert(Bee),
            EntityType::Blaze => entity.insert(Blaze),
            EntityType::Bogged => entity.insert(Bogged),
            EntityType::Breeze => entity.insert(Breeze),
            EntityType::Camel => entity.insert(Camel),
            EntityType::Cat => entity.insert(Cat),
            EntityType::CaveSpider => entity.insert(CaveSpider),
            EntityType::Chicken => entity.insert(Chicken),
            EntityType::Cod => entity.insert(Cod),
            EntityType::Cow => entity.insert(Cow),
            EntityType::Creaking => entity.insert(Creaking),
            EntityType::Creeper => entity.insert(Creeper),
            EntityType::Dolphin => entity.insert(Dolphin),
            EntityType::Donkey => entity.insert(Donkey),
            EntityType::Drowned => entity.insert(Drowned),
            EntityType::ElderGuardian => entity.insert(ElderGuardian),
            EntityType::Enderman => entity.insert(Enderman),
            EntityType::Endermite => entity.insert(Endermite),
            EntityType::Evoker => entity.insert(Evoker),
            EntityType::Fox => entity.insert(Fox),
            EntityType::Frog => entity.insert(Frog),
            EntityType::Ghast => entity.insert(Ghast),
            EntityType::GlowSquid => entity.insert(GlowSquid),
            EntityType::Goat => entity.insert(Goat),
            EntityType::Guardian => entity.insert(Guardian),
            EntityType::Hoglin => entity.insert(Hoglin),
            EntityType::Horse => entity.insert(Horse),
            EntityType::Husk => entity.insert(Husk),
            EntityType::IronGolem => entity.insert(IronGolem),
            EntityType::Llama => entity.insert(Llama),
            EntityType::MagmaCube => entity.insert(MagmaCube),
            EntityType::Mooshroom => entity.insert(Mooshroom),
            EntityType::Mule => entity.insert(Mule),
            EntityType::Ocelot => entity.insert(Ocelot),
            EntityType::Panda => entity.insert(Panda),
            EntityType::Parrot => entity.insert(Parrot),
            EntityType::Phantom => entity.insert(Phantom),
            EntityType::Pig => entity.insert(Pig),
            EntityType::Piglin => entity.insert(Piglin),
            EntityType::PiglinBrute => entity.insert(PiglinBrute),
            EntityType::Pillager => entity.insert(Pillager),
            EntityType::PolarBear => entity.insert(PolarBear),
            EntityType::Pufferfish => entity.insert(Pufferfish),
            EntityType::Rabbit => entity.insert(Rabbit),
            EntityType::Ravager => entity.insert(Ravager),
            EntityType::Salmon => entity.insert(Salmon),
            EntityType::Sheep => entity.insert(Sheep),
            EntityType::Shulker => entity.insert(Shulker),
            EntityType::Silverfish => entity.insert(Silverfish),
            EntityType::Skeleton => entity.insert(Skeleton),
            EntityType::SkeletonHorse => entity.insert(SkeletonHorse),
            EntityType::Slime => entity.insert(Slime),
            EntityType::Sniffer => entity.insert(Sniffer),
            EntityType::SnowGolem => entity.insert(SnowGolem),
            EntityType::Spider => entity.insert(Spider),
            EntityType::Squid => entity.insert(Squid),
            EntityType::Stray => entity.insert(Stray),
            EntityType::Strider => entity.insert(Strider),
            EntityType::Tadpole => entity.insert(Tadpole),
            EntityType::TraderLlama => entity.insert(TraderLlama),
            EntityType::TropicalFish => entity.insert(TropicalFish),
            EntityType::Turtle => entity.insert(Turtle),
            EntityType::Vex => entity.insert(Vex),
            EntityType::Villager => entity.insert(Villager),
            EntityType::Vindicator => entity.insert(Vindicator),
            EntityType::WanderingTrader => entity.insert(WanderingTrader),
            EntityType::Warden => entity.insert(Warden),
            EntityType::Witch => entity.insert(Witch),
            EntityType::WitherSkeleton => entity.insert(WitherSkeleton),
            EntityType::Wolf => entity.insert(Wolf),
            EntityType::Zoglin => entity.insert(Zoglin),
            EntityType::Zombie => entity.insert(Zombie),
            EntityType::ZombieHorse => entity.insert(ZombieHorse),
            EntityType::ZombieVillager => entity.insert(ZombieVillager),
            EntityType::ZombifiedPiglin => entity.insert(ZombifiedPiglin),
            _ => entity.insert(()),
        };

        let id = entity.id();
        commands.queue(move |world: &mut World| {
            broadcast_entity_spawn(world, id);
        });
    }
}
