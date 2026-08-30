//! What an entity carries into a fight.

use bevy_ecs::prelude::Component;

/// The part of a fight that is state rather than kind.
///
/// Whether a thing may be attacked at all is decided by its class in vanilla — a marker and an
/// area effect cloud refuse — which is Phase 7's to write; until then everything may be.
#[derive(Component, Clone, Copy, Debug)]
pub struct CombatProperties {
    /// True if an entity is attackable
    ///
    /// Some entity (area_effect_cloud, markers) can not be attackable.
    /// This value come from vanilla data and normally do not change.
    pub attackable: bool,

    /// Number of invulnerability_ticks left
    ///
    /// When an entity take damage, she become invincible for a short time
    /// (generally 10 ticks = 0.5 seconds) to preserve entity from multiple
    /// hits
    ///
    /// This count is decremented every tick and the entity can't be damaged
    /// while (count > 0)
    pub invulnerability_ticks: u32,
}

impl Default for CombatProperties {
    fn default() -> Self {
        Self {
            attackable: true,
            invulnerability_ticks: 0,
        }
    }
}

impl CombatProperties {
    /// Standard invulnerability duration after a hit in ticks.
    ///
    /// In vanilla Minecraft, it's 10 ticks (0.5 seconds).
    pub const DEFAULT_INVULNERABILITY_TICKS: u32 = 10;

    /// Return true if the entity can't be damaged.
    pub const fn can_be_damaged(&self) -> bool {
        self.attackable && self.invulnerability_ticks == 0
    }

    /// Activate invulnerability for a certain amount of ticks.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut combat = CombatProperties::from_vanilla(&VanillaEntityType::PIG);
    /// combat.set_invulnerable(10);
    /// assert!(!combat.can_be_damaged());
    /// ```
    pub fn set_invulnerable(&mut self, ticks: u32) {
        self.invulnerability_ticks = ticks;
    }

    /// Activate default duration invulnerability
    pub fn set_default_invulnerability(&mut self) {
        self.set_invulnerable(Self::DEFAULT_INVULNERABILITY_TICKS);
    }

    /// Decream invulnerability count (call every ticks)
    ///
    /// # Return
    ///
    /// True if the entity was invulnerable and is no longer.
    pub fn tick(&mut self) -> bool {
        if self.invulnerability_ticks > 0 {
            self.invulnerability_ticks -= 1;
            self.invulnerability_ticks == 0
        } else {
            false
        }
    }

    /// Remove immediatly invulnerability
    pub fn clear_invulnerability(&mut self) {
        self.invulnerability_ticks = 0;
    }
}
