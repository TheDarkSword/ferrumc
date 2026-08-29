//! Place Recipe packet: a click in the recipe book asking for the ingredients to be laid out.

use ferrumc_macros::{packet, NetDecode};
use ferrumc_net_codec::net_types::var_int::VarInt;

#[derive(NetDecode, Debug)]
#[packet(packet_id = "place_recipe", state = "play")]
pub struct PlaceRecipe {
    pub window_id: VarInt,
    /// Index into the recipes this player has been sent, not a registry id.
    pub recipe: VarInt,
    /// Whether to lay out as many as the ingredients allow rather than one.
    pub use_max_items: bool,
}
