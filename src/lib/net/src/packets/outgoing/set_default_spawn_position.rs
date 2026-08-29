use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::network_position::NetworkPosition;

#[derive(NetEncode)]
#[downgrade_with(crate::translate::to_1_21_7::set_default_spawn_position)]
#[packet(packet_id = "set_default_spawn_position", state = "play")]
pub struct SetDefaultSpawnPositionPacket {
    /// 1.21.9 put the spawn in a named dimension rather than implying the one being played.
    pub dimension: String,
    pub spawn_position: NetworkPosition,
    pub yaw: f32,
    /// Added alongside the dimension; older clients face wherever the yaw puts them.
    pub pitch: f32,
}

const OVERWORLD: &str = "minecraft:overworld";

// Spawn in chunk (1, 1) at y=100 to ensure spawning above ground, since for some reason the terrain
// gen can't create land at (0, 0)
pub const DEFAULT_SPAWN_POSITION: NetworkPosition = NetworkPosition {
    x: 16,
    y: 100,
    z: 16,
};

const DEFAULT_ANGLE: f32 = 0.0;

impl Default for SetDefaultSpawnPositionPacket {
    fn default() -> Self {
        Self::new()
    }
}

impl SetDefaultSpawnPositionPacket {
    pub fn new() -> Self {
        Self {
            dimension: OVERWORLD.to_string(),
            spawn_position: DEFAULT_SPAWN_POSITION,
            yaw: DEFAULT_ANGLE,
            pitch: DEFAULT_ANGLE,
        }
    }
}
