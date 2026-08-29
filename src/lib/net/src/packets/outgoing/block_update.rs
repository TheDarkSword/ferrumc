use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::network_position::NetworkPosition;
use ferrumc_world::chunk::remap::NetworkBlockState;

#[derive(NetEncode)]
#[packet(packet_id = "block_update", state = "play")]
pub struct BlockUpdate {
    pub location: NetworkPosition,
    /// Translated per connection as it is written; a block update is broadcast to players who need
    /// not share a protocol version.
    pub block_state_id: NetworkBlockState,
}
