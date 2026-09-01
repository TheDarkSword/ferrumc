use crate::{
    active_effects::ActiveEffects,
    health::Health,
    player::{
        abilities::PlayerAbilities, experience::Experience, gamemode::GameModeComponent,
        gameplay_state::ender_chest::EnderChest, hunger::Hunger,
    },
};
use bevy_ecs::prelude::Bundle;
use ferrumc_core::{
    chunks::chunk_receiver::ChunkReceiver,
    identity::player_identity::PlayerIdentity,
    transform::{grounded::OnGround, position::Position, rotation::Rotation},
};
use ferrumc_damage::{Defence, Reeling, Swing, Vitals};
use ferrumc_inventories::{hotbar::Hotbar, inventory::Inventory};
/// A Bevy Bundle containing all components required for a player entity.
/// This groups all 17+ components into a single, spawnable unit.
#[derive(Bundle, Default)]
pub struct PlayerBundle {
    // Identity
    pub identity: PlayerIdentity,

    // Core State
    pub abilities: PlayerAbilities,
    pub gamemode: GameModeComponent,

    // Position/World
    pub position: Position,
    pub rotation: Rotation,
    pub on_ground: OnGround,
    pub chunk_receiver: ChunkReceiver,

    // Inventory
    pub inventory: Inventory,
    pub hotbar: Hotbar,
    pub ender_chest: EnderChest,

    // Survival Stats
    pub health: Health,
    /// What softens a blow, how long the last one is still felt, and the counters that turn
    /// falling, drowning and burning into blows in the first place.
    pub defence: Defence,
    pub reeling: Reeling,
    pub vitals: Vitals,
    pub swing: Swing,
    pub hunger: Hunger,
    pub experience: Experience,
    pub active_effects: ActiveEffects,
}
