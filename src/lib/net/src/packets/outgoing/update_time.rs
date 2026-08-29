use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::length_prefixed_vec::LengthPrefixedVec;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::net_types::var_long::VarLong;

/// Index of `minecraft:overworld` in the `minecraft:world_clock` registry. Entries of a synced
/// registry are numbered in the order they are sent, which is alphabetical, and the registry holds
/// only `overworld` and `the_end`.
const OVERWORLD_CLOCK: VarInt = VarInt(0);

/// Rate a running clock advances at. Zero freezes it, which is how a paused day cycle is expressed.
const RUNNING: f32 = 1.0;
const FROZEN: f32 = 0.0;

#[derive(NetEncode)]
#[packet(packet_id = "set_time", state = "play")]
#[downgrade_with(crate::translate::to_1_21_11::set_time)]
pub struct UpdateTimePacket {
    /// Monotonic total game-tick count. TPS readouts such as MiniHUD derive the server tick rate
    /// from the delta between consecutive packets, so this has to advance with real ticks.
    pub game_time: i64,
    pub clock_updates: LengthPrefixedVec<ClockUpdate>,
}

/// One entry of the clock map: which clock, where it stands, and how fast it is running.
#[derive(NetEncode, Clone)]
pub struct ClockUpdate {
    pub clock: VarInt,
    pub total_ticks: VarLong,
    pub partial_tick: f32,
    pub rate: f32,
}

impl UpdateTimePacket {
    /// Full update for the overworld clock, as sent on join and whenever the time is resynced.
    pub fn overworld(game_time: i64, time_of_day: i64, advancing: bool) -> Self {
        Self {
            game_time,
            clock_updates: LengthPrefixedVec::new(vec![ClockUpdate {
                clock: OVERWORLD_CLOCK,
                total_ticks: VarLong::new(time_of_day),
                partial_tick: 0.0,
                rate: if advancing { RUNNING } else { FROZEN },
            }]),
        }
    }
}
