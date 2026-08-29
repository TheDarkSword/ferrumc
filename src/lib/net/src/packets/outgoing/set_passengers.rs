//! Set Passengers packet: who is riding what.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::length_prefixed_vec::LengthPrefixedVec;
use ferrumc_net_codec::net_types::var_int::VarInt;

#[derive(NetEncode)]
#[packet(packet_id = "set_passengers", state = "play")]
pub struct SetPassengers {
    pub vehicle: VarInt,
    /// Everyone aboard, in seating order. Empty means the vehicle was left.
    pub passengers: LengthPrefixedVec<VarInt>,
}
