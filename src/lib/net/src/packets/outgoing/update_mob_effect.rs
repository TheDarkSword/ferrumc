//! Update Mob Effect packet: an effect applied or refreshed.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::registry_remap::NetworkMobEffect;

#[derive(NetEncode)]
#[packet(packet_id = "update_mob_effect", state = "play")]
pub struct UpdateMobEffect {
    pub entity_id: VarInt,
    pub effect: NetworkMobEffect,
    /// Level above the first, so zero is Strength I.
    pub amplifier: VarInt,
    /// Ticks remaining. A negative duration is one that does not run out.
    pub duration: VarInt,
    /// Ambient, visible, shows an icon, blends with the sky: one bit each, in that order.
    pub flags: u8,
}

/// From a beacon or a conduit, which a client draws faintly.
const AMBIENT: u8 = 1;
/// Whether the swirling particles are shown.
const VISIBLE: u8 = 2;
/// Whether the icon is shown in the corner.
const SHOW_ICON: u8 = 4;
/// Whether the colour fades in and out rather than appearing outright.
const BLEND: u8 = 8;

/// How an effect is drawn, which the wire packs into one byte.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Shown {
    pub ambient: bool,
    pub visible: bool,
    pub show_icon: bool,
    pub blend: bool,
}

impl Shown {
    /// The byte the wire carries.
    #[must_use]
    pub const fn flags(self) -> u8 {
        let mut flags = 0;
        if self.ambient {
            flags |= AMBIENT;
        }
        if self.visible {
            flags |= VISIBLE;
        }
        if self.show_icon {
            flags |= SHOW_ICON;
        }
        if self.blend {
            flags |= BLEND;
        }
        flags
    }
}

impl UpdateMobEffect {
    #[must_use]
    pub const fn new(
        entity_id: i32,
        effect: u32,
        amplifier: u8,
        duration: i32,
        shown: Shown,
    ) -> Self {
        Self {
            entity_id: VarInt::new(entity_id),
            effect: NetworkMobEffect(effect),
            amplifier: VarInt::new(amplifier as i32),
            duration: VarInt::new(duration),
            flags: shown.flags(),
        }
    }
}
