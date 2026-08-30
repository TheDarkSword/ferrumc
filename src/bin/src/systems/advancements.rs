//! Keeping track of what each player has done.
//!
//! A player's advancements are read when they join and written back when they leave, in a table of
//! their own — as vanilla keeps them in a file of their own — so adding to one cannot cost them
//! the rest of their data.
//!
//! Gameplay offers events to the criteria waiting for them. Only the ones whose gameplay exists
//! are offered anything; the rest are read and never fire.

use bevy_ecs::prelude::{Changed, Commands, Component, Entity, MessageReader, Query, Res};
use ferrumc_advancements::trigger::Carried;
use ferrumc_advancements::{PlayerAdvancements, Trigger};
use ferrumc_core::identity::player_identity::PlayerIdentity;
use ferrumc_inventories::inventory::Inventory;
use ferrumc_messages::player_join::PlayerJoined;
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::update_advancements::UpdateAdvancementsPacket;
use ferrumc_state::GlobalStateResource;
use ferrumc_world::player::PLAYER_ADVANCEMENTS;
use tracing::{trace, warn};

/// What one player has done, kept on their entity.
#[derive(Component, Debug, Default)]
pub struct Advancement(pub PlayerAdvancements);

/// Reads a player's advancements when they join, and tells them the whole tree.
pub fn on_join(
    mut joined: MessageReader<PlayerJoined>,
    mut commands: Commands,
    state: Res<GlobalStateResource>,
    packs: Res<crate::systems::datapacks::Datapacks>,
    writers: Query<&StreamWriter>,
) {
    for event in joined.read() {
        let progress: PlayerAdvancements = state
            .0
            .world
            .load_player_table(PLAYER_ADVANCEMENTS, event.identity.uuid)
            .unwrap_or_else(|err| {
                warn!(
                    "Failed to read advancements for {}: {err:?}",
                    event.identity.username
                );
                None
            })
            .unwrap_or_default();

        send(&writers, event.entity, |_| {
            UpdateAdvancementsPacket::everything(&packs.advancements, &progress)
        });
        commands.entity(event.entity).insert(Advancement(progress));
    }
}

/// Offers what a player is carrying to every criterion waiting on it.
pub fn on_inventory_change(
    mut changed: Query<(Entity, &Inventory, &mut Advancement), Changed<Inventory>>,
    packs: Res<crate::systems::datapacks::Datapacks>,
    writers: Query<&StreamWriter>,
) {
    for (entity, inventory, mut advancements) in changed.iter_mut() {
        let slots = carried(inventory);
        let carried = Carried { slots: &slots };
        let now = now();
        let granted = advancements.0.offer(&packs.advancements, now, |trigger| {
            matches!(trigger, Trigger::InventoryChanged { .. }) && trigger.inventory_meets(&carried)
        });
        // Most changes to what a player carries grant nothing, and the screen only needs telling
        // when something moved.
        if granted.is_empty() {
            continue;
        }
        for name in &granted.completed {
            trace!("{entity:?} finished {name}");
        }
        send(&writers, entity, |_| {
            UpdateAdvancementsPacket::changed(&advancements.0)
        });
    }
}

/// Writes what a player has done back, which the disconnect and sync paths both do.
pub fn save(state: &GlobalStateResource, identity: &PlayerIdentity, progress: &PlayerAdvancements) {
    if let Err(err) = state
        .0
        .world
        .save_player_table(PLAYER_ADVANCEMENTS, identity.uuid, progress)
    {
        warn!(
            "Failed to save advancements for {}: {err:?}",
            identity.username
        );
    }
}

/// Every slot of what a player carries, as an item and how many.
fn carried(inventory: &Inventory) -> Vec<Option<(i32, i32)>> {
    inventory
        .slots
        .iter()
        .map(|slot| {
            let slot = slot.as_ref()?;
            Some((slot.item_id?.0 .0, slot.count.0))
        })
        .collect()
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| i64::try_from(since.as_millis()).unwrap_or_default())
        .unwrap_or_default()
}

fn send(
    writers: &Query<&StreamWriter>,
    entity: Entity,
    packet: impl FnOnce(Entity) -> UpdateAdvancementsPacket,
) {
    let Ok(writer) = writers.get(entity) else {
        return;
    };
    if let Err(err) = writer.send_packet_ref(&packet(entity)) {
        warn!("Failed to send advancements to {entity:?}: {err:?}");
    }
}
